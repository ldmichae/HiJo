#![no_std]
#![no_main]

mod gps;
mod utils;
mod storage;
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, channel::Channel, mutex::Mutex};
use embassy_time::{Duration, Timer};
use embedded_sdmmc::{VolumeIdx};
use panic_halt as _;
use static_cell::StaticCell;

use crate::{
    draw_fns::utils::{
        draw_blinky, draw_coords, draw_current_speed, draw_hdop, draw_recording_status, draw_static_text, draw_storage_status, draw_total_distance, draw_total_elev_gain
    },
    gps::{
        reader::{GpsReader, ParseOut},
        stack::GeoStack,
    }, storage::sd::{self, SetupPins, WriteableDirectory},
};

use embassy_executor::Spawner;
use embassy_nrf::{
    bind_interrupts, gpio::{Input, Pull}, peripherals::{self}, spim, twim::{self, Twim}, uarte::{self, Baudrate, Parity}
};
use nmea::sentences::FixType;

use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_10X20, iso_8859_15::FONT_5X8, ascii::FONT_6X13, ascii::FONT_4X6},
    pixelcolor::BinaryColor,
    prelude::*,
};

use ssd1306::{I2CDisplayInterface, Ssd1306, prelude::*};
mod draw_fns;
bind_interrupts!(struct Irqs {
    TWISPI0 => twim::InterruptHandler<peripherals::TWISPI0>;
    UARTE0 => uarte::InterruptHandler<peripherals::UARTE0>;
    SPI2 => spim::InterruptHandler<peripherals::SPI2>;
});

static CHANNEL: StaticCell<Channel<NoopRawMutex, ParseOut, 1>> = StaticCell::new();
static RECORDING_STATE: StaticCell<Mutex<NoopRawMutex, bool>> = StaticCell::new();
static BLINK_STATE: StaticCell<Mutex<NoopRawMutex, bool>> = StaticCell::new();
static STORAGE_STATE: StaticCell<Mutex<NoopRawMutex, bool>> = StaticCell::new();

static GPS_READER: StaticCell<GpsReader<'static>> = StaticCell::new();

const TEXT_STYLE_LG: MonoTextStyle<'_, BinaryColor> =
    MonoTextStyle::new(&FONT_10X20, BinaryColor::On);
const TEXT_STYLE_MD: MonoTextStyle<'_, BinaryColor> =
    MonoTextStyle::new(&FONT_6X13, BinaryColor::On);
const TEXT_STYLE_SM: MonoTextStyle<'_, BinaryColor> =
    MonoTextStyle::new(&FONT_5X8, BinaryColor::On);
const TEXT_STYLE_XS: MonoTextStyle<'_, BinaryColor> =
    MonoTextStyle::new(&FONT_4X6, BinaryColor::On);

pub struct SharedState {
    pub is_recording: bool,
}

#[embassy_executor::task]
async fn gps_reader_task(gps_reader: &'static mut GpsReader<'static>) {
    gps_reader.run().await;
}

#[embassy_executor::task]
async fn button_task(button: Input<'static>, shared: &'static Mutex<NoopRawMutex, bool>) {
    let mut last_state = button.is_high(); // true if not pressed

    loop {
        let current_state = button.is_high(); // still true if not pressed

        if last_state && !current_state {
            // falling edge: just pressed
            // debounce
            Timer::after(Duration::from_millis(20)).await;

            if button.is_low() {
                // confirmed press
                let mut lock = shared.lock().await;
                *lock = !*lock;
            }
        }

        last_state = current_state;
        Timer::after(Duration::from_millis(10)).await;
    }
}

// Add this task function
#[embassy_executor::task]
async fn show_jo_updater_task(show_jo_mutex: &'static Mutex<NoopRawMutex, bool>) {
    loop {
        // Toggle the show_jo state
        let mut show_jo_lock = show_jo_mutex.lock().await;
        *show_jo_lock = !*show_jo_lock;
        drop(show_jo_lock); // Release the lock immediately

        // Wait for 1 second before the next toggle
        Timer::after(Duration::from_millis(500)).await;
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let recording_mutex_ref = RECORDING_STATE.init(Mutex::new(true));
    let blink_mutex_ref = BLINK_STATE.init(Mutex::new(true));
    let storage_mutex_ref = STORAGE_STATE.init(Mutex::new(true));

    let p = embassy_nrf::init(Default::default());

    // set up uarte
    let mut uart_config = uarte::Config::default();
    uart_config.parity = Parity::EXCLUDED;
    uart_config.baudrate = Baudrate::BAUD115200;

    let uart = uarte::Uarte::new(p.UARTE0, Irqs, p.P0_08, p.P0_06, uart_config);

    // set up sd card
    let volume_mgr = sd::setup(SetupPins {
        sck: p.P1_11,
        miso: p.P1_13,
        mosi: p.P1_15,
        spi2: p.SPI2,
        output: p.P0_02,
    });

    // set up display
    let twim_config = twim::Config::default();
    let i2c = Twim::new(p.TWISPI0, Irqs, p.P0_22, p.P0_24, twim_config);

    let interface = I2CDisplayInterface::new(i2c);
    let mut display = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
        .into_buffered_graphics_mode();
    display.init().unwrap();

    let gps_channel = CHANNEL.init(Channel::new());
    let gps_reader = GPS_READER.init(GpsReader::new(uart, gps_channel.sender()));

    let button = Input::new(p.P0_17, Pull::Up);

    let mut last_fix: Option<FixType> = None;
    let mut last_lat_lon_alt: Option<gps::reader::GpsReaderResults> = None;
    let mut geo_stack = GeoStack::new();

    let mut writeable_directory: WriteableDirectory;

    spawner.spawn(button_task(button, recording_mutex_ref)).unwrap();
    spawner.spawn(gps_reader_task(gps_reader)).unwrap();
    spawner.spawn(show_jo_updater_task(blink_mutex_ref)).unwrap();

    loop {
        let is_recording = {
            let lock = recording_mutex_ref.lock().await;
            *lock
        };

        let should_blink = {
            let lock = blink_mutex_ref.lock().await;
            *lock
        };
        let is_storage_configured = {
            let lock = storage_mutex_ref.lock().await;
            *lock
        };

        Timer::after(Duration::from_millis(100)).await;
        display.clear(BinaryColor::Off).unwrap();

        draw_static_text(&mut display, TEXT_STYLE_LG).unwrap();
        if should_blink {
            draw_blinky(&mut display);
        }

        let volume_open_attempt = volume_mgr.open_volume(VolumeIdx(0));
        match volume_open_attempt {
            Ok(v) => {
                let mut storage_lock = storage_mutex_ref.lock().await;
                *storage_lock = true;
                drop(storage_lock);

                let root_dir = volume_mgr.open_root_dir(v.to_raw_volume()).expect("Could not open root directory");
                writeable_directory = root_dir.to_directory(&volume_mgr);
            }
            Err(_) => {
                let mut storage_lock = storage_mutex_ref.lock().await;
                *storage_lock = false;
                drop(storage_lock);
            }
        }

        while let Ok(gps_parse) = gps_channel.receiver().try_receive() {
            last_fix = gps_parse.fix.or(last_fix);
            let new_coords = gps_parse.reader_results.or(last_lat_lon_alt);
            last_lat_lon_alt = new_coords;
            if let Some(coords) = new_coords {
                geo_stack.add_coords(coords, is_recording);
            }
        }

        draw_recording_status(is_recording, &mut display);
        draw_hdop(last_fix, geo_stack.current_hdop, &mut display);
        draw_storage_status(is_storage_configured, &mut display);

        draw_coords(&last_lat_lon_alt, &mut display);
        draw_total_elev_gain(geo_stack.total_elevation_gain.into(), &mut display);
        draw_total_distance(geo_stack.total_distance, &mut display);
        draw_current_speed(geo_stack.current_speed_mph, &mut display);

        display.flush().unwrap();
    }
}
