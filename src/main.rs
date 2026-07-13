#![no_std]
#![no_main]

mod gps;
mod utils;
use embassy_futures::select::{Either, select};
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, channel::Channel, mutex::Mutex};
use embassy_time::{Duration, Timer};
use static_cell::StaticCell;

use defmt_rtt as _;
use panic_probe as _;

use crate::{
    draw_fns::utils::{
        draw_blinky, draw_coords, draw_current_speed, draw_hdop, draw_recording_status,
        draw_static_text, draw_total_distance, draw_total_elev_gain,
    },
    gps::{
        reader::{GpsReader, ParseOut},
        stack::GeoStack,
    },
};

use embassy_executor::Spawner;
use embassy_nrf::{
    bind_interrupts, buffered_uarte::{self, BufferedUarte}, gpio::{Input, Pull}, peripherals::{self}, twim::{self, Twim}, uarte::{self, Baudrate, Parity},
};
use nmea::sentences::FixType;

use embedded_graphics::{
    mono_font::{
        MonoTextStyle, ascii::FONT_4X6, ascii::FONT_6X13, ascii::FONT_10X20, iso_8859_15::FONT_5X8,
    },
    pixelcolor::BinaryColor,
    prelude::*,
};

use ssd1306::{I2CDisplayInterface, Ssd1306, prelude::*};
mod draw_fns;
bind_interrupts!(struct Irqs {
    SERIAL0 => twim::InterruptHandler<peripherals::SERIAL0>;
    SERIAL1 => buffered_uarte::InterruptHandler<peripherals::SERIAL1>;
});

static CHANNEL: StaticCell<Channel<NoopRawMutex, ParseOut, 1>> = StaticCell::new();
static SHARED_STATE: StaticCell<Mutex<NoopRawMutex, SharedState>> = StaticCell::new();
static BLINK_STATE: StaticCell<Mutex<NoopRawMutex, bool>> = StaticCell::new();

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
async fn button_task(button: Input<'static>, shared: &'static Mutex<NoopRawMutex, SharedState>) {
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
                lock.is_recording = !lock.is_recording;
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

static RX_BUFFER: StaticCell<[u8; 256]> = StaticCell::new();
static TX_BUFFER: StaticCell<[u8; 256]> = StaticCell::new();

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let shared = SHARED_STATE.init(Mutex::new(SharedState {
        is_recording: false,
    }));
    let blink_mutex_ref = BLINK_STATE.init(Mutex::new(true));

    let p = embassy_nrf::init(Default::default());

    // set up uarte
    let mut uart_config = uarte::Config::default();
    uart_config.parity = Parity::Excluded;
    uart_config.baudrate = Baudrate::Baud115200;

    let rx_buffer = RX_BUFFER.init([0u8; 256]);
    let tx_buffer = TX_BUFFER.init([0u8; 256]);

    let uart = BufferedUarte::new(
        p.SERIAL1,
        p.TIMER0,
        p.PPI_CH1,
        p.PPI_CH2,
        p.PPI_GROUP0,
        p.P0_08,
        p.P0_06,
        Irqs,
        uart_config,
        &mut rx_buffer[..],
        &mut tx_buffer[..]);

    let serial_port = p.SERIAL0;
    let sda_pin = p.P1_12;
    let scl_pin = p.P1_14;

    // set up display
    let twim_config = twim::Config::default();
    let mut tx_ram_buffer: [u8; 64] = [0; 64];
    let my_twim = Twim::new(
        serial_port,
        Irqs,
        sda_pin,
        scl_pin,
        twim_config,
        &mut tx_ram_buffer,
    );
    let interface = I2CDisplayInterface::new(my_twim);
    let mut display: Ssd1306<
        ssd1306::prelude::I2CInterface<Twim<'_>>,
        DisplaySize128x64,
        ssd1306::mode::BufferedGraphicsMode<DisplaySize128x64>,
    > = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
        .into_buffered_graphics_mode();
    display.init().unwrap();

    let gps_channel = CHANNEL.init(Channel::new());
    let gps_receiver = gps_channel.receiver();
    let gps_reader = GPS_READER.init(GpsReader::new(uart, gps_channel.sender()));

    let button = Input::new(p.P0_23, Pull::Up);

    let mut last_fix: Option<FixType> = None;

    let mut last_lat_lon_alt: Option<gps::reader::GpsReaderResults> = None;

    let mut geo_stack = GeoStack::new();

    spawner.spawn(button_task(button, shared).unwrap());
    spawner.spawn(gps_reader_task(gps_reader).unwrap());
    spawner.spawn(show_jo_updater_task(blink_mutex_ref).unwrap());

    loop {
        let is_recording = {
            let lock = shared.lock().await;
            lock.is_recording
        };

        let should_blink = {
            let lock = blink_mutex_ref.lock().await;
            *lock
        };

        let draw_future = Timer::after(Duration::from_millis(100));
        let gps_future = gps_receiver.receive();
        match select(draw_future, gps_future).await {
            Either::First(_) => {
                display.clear(BinaryColor::Off).unwrap();

                draw_static_text(&mut display, TEXT_STYLE_LG).unwrap();
                if should_blink {
                    draw_blinky(&mut display);
                }
                draw_coords(&last_lat_lon_alt, &mut display);

                draw_recording_status(is_recording, &mut display);
                draw_total_elev_gain(geo_stack.total_elevation_gain.into(), &mut display);
                draw_total_distance(geo_stack.total_distance, &mut display);
                draw_current_speed(geo_stack.current_speed_mph, &mut display);
                draw_hdop(last_fix, geo_stack.current_hdop, &mut display);

                display.flush().unwrap();
            }
            Either::Second(gps_parse) => {
                last_fix = gps_parse.fix.or(last_fix);
                let new_coords = gps_parse.reader_results.or(last_lat_lon_alt);
                last_lat_lon_alt = new_coords;
                if let Some(coords) = new_coords {
                    geo_stack.add_coords(coords, is_recording);
                }
            }
        }
    }
}
