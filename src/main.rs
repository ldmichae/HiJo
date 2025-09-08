#![no_std]
#![no_main]

mod gps;
mod draw_fns;
mod utils;

use embassy_sync::{blocking_mutex::raw::NoopRawMutex, channel::Channel, mutex::Mutex};
use embassy_time::{Duration, Timer};
use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_10X20, iso_8859_15::FONT_5X8, ascii::FONT_6X13, ascii::FONT_4X6},
    pixelcolor::BinaryColor,
    prelude::*,
};

use ssd1306::{I2CDisplayInterface, Ssd1306, prelude::*};
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};
use defmt;

use crate::{
    draw_fns::utils::{draw_blinky, draw_coords, draw_current_speed, draw_datetime, draw_hdop, draw_recording_status, draw_static_text, draw_total_distance, draw_total_elev_gain}, gps::{
        reader::{GpsReader, ParseOut},
        stack::GeoStack,
    }
};

use embassy_executor::Spawner;
use embassy_nrf::{
    bind_interrupts, gpio::{Input, Pull}, peripherals, twim::{self, Twim}, uarte::{self, Baudrate, Parity}
};
use nmea::sentences::FixType;

bind_interrupts!(struct Irqs {
    SERIAL0 => twim::InterruptHandler<peripherals::SERIAL0>;
    SERIAL1 => uarte::InterruptHandler<peripherals::SERIAL1>;
});

static CHANNEL: StaticCell<Channel<NoopRawMutex, ParseOut, 1>> = StaticCell::new();

static GPS_READER: StaticCell<GpsReader<'static>> = StaticCell::new();

static RECORDING_STATE: StaticCell<Mutex<NoopRawMutex, bool>> = StaticCell::new();
static BLINK_STATE: StaticCell<Mutex<NoopRawMutex, bool>> = StaticCell::new();

const TEXT_STYLE_LG: MonoTextStyle<'_, BinaryColor> =
    MonoTextStyle::new(&FONT_10X20, BinaryColor::On);
const TEXT_STYLE_MD: MonoTextStyle<'_, BinaryColor> =
    MonoTextStyle::new(&FONT_6X13, BinaryColor::On);
const TEXT_STYLE_SM: MonoTextStyle<'_, BinaryColor> =
    MonoTextStyle::new(&FONT_5X8, BinaryColor::On);
const TEXT_STYLE_XS: MonoTextStyle<'_, BinaryColor> =
    MonoTextStyle::new(&FONT_4X6, BinaryColor::On);

#[embassy_executor::task]
async fn gps_reader_task(gps_reader: &'static mut GpsReader<'static>) {
    gps_reader.run().await;
}

#[embassy_executor::task]
async fn blinky_task(show_jo_mutex: &'static Mutex<NoopRawMutex, bool>) {
    loop {
        // Toggle the show_jo state
        let mut show_jo_lock = show_jo_mutex.lock().await;
        *show_jo_lock = !*show_jo_lock;
        drop(show_jo_lock); // Release the lock immediately

        // Wait for 1 second before the next toggle
        Timer::after(Duration::from_millis(500)).await;
    }
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

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_nrf::init(Default::default());
    let blink_mutex_ref = BLINK_STATE.init(Mutex::new(true));

    // set up buttons
    let recording_state =  RECORDING_STATE.init(Mutex::new(false));
    let start_stop_button = Input::new(p.P0_23, Pull::Up);

    // set up display
    let twim_config = twim::Config::default();
    let i2c = Twim::new(p.SERIAL0, Irqs, p.P0_26, p.P0_25, twim_config, &mut[]);

    let interface = I2CDisplayInterface::new(i2c);
    let mut display = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
        .into_buffered_graphics_mode();
    display.init().unwrap();

    // set up uarte
    let mut uart_config = uarte::Config::default();
    uart_config.parity = Parity::EXCLUDED;
    uart_config.baudrate = Baudrate::BAUD115200;

    let uart = uarte::Uarte::new(p.SERIAL1, p.P0_08, p.P0_06, Irqs, uart_config);

    let gps_channel = CHANNEL.init(Channel::new());
    let gps_reader = GPS_READER.init(GpsReader::new(uart, gps_channel.sender()));

    let mut last_fix: Option<FixType> = None;

    let mut last_lat_lon_alt: Option<gps::reader::GpsReaderResults> = None;
    let mut last_datetime: Option<gps::reader::InternalDateDTO> = None;

    let mut geo_stack = GeoStack::new();

    spawner.spawn(gps_reader_task(gps_reader)).unwrap();
    spawner.spawn(button_task(start_stop_button, recording_state)).unwrap();
    spawner.spawn(blinky_task(blink_mutex_ref)).unwrap();

    loop {
        let is_recording = {
            let lock = recording_state.lock().await;
            *lock
        };

        let should_blink = {
            let lock = blink_mutex_ref.lock().await;
            *lock
        };


        Timer::after(Duration::from_millis(100)).await;
        display.clear(BinaryColor::Off).unwrap();

        if should_blink {
            draw_blinky(&mut display);
        }

        draw_static_text(&mut display, TEXT_STYLE_MD).unwrap();

        while let Ok(gps_parse) = gps_channel.receiver().try_receive() {
            if let Some(dt) = gps_parse.reader_datetime {
                defmt::info!("ZDA SENTENCE PARSED {} - {}", dt.pretty_date.as_str(), dt.pretty_time.as_str());
                last_datetime = Some(dt);
            } else {
                defmt::info!("NO SIGNAL AT ALL {}", gps_parse.line.as_str());
            }
            last_fix = gps_parse.fix.or(last_fix);
            let new_coords = gps_parse.reader_results.or(last_lat_lon_alt);
            last_lat_lon_alt = new_coords;
            if let Some(coords) = new_coords {
                geo_stack.add_coords(coords, true);
            }
        }

        draw_coords(&last_lat_lon_alt, &mut display);

        draw_recording_status(is_recording, &mut display);
        draw_total_elev_gain(geo_stack.total_elevation_gain.into(), &mut display);
        draw_total_distance(geo_stack.total_distance, &mut display);
        draw_current_speed(geo_stack.current_speed_mph, &mut display);
        draw_hdop(last_fix, geo_stack.current_hdop, &mut display);
        draw_datetime(&last_datetime, &mut display);

        display.flush().unwrap();
    }
}
