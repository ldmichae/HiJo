#![no_std]
#![no_main]

mod gps;
mod utils;
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, channel::Channel, mutex::Mutex};
use embassy_time::{Duration, Timer};
use panic_halt as _;
use static_cell::StaticCell;

use crate::{
    draw_fns::utils::{
        draw_coords, draw_current_speed, draw_fix_status, draw_hdop, draw_last_segment_distance,
        draw_moving_jo, draw_recording_status, draw_static_text, draw_total_distance,
    },
    gps::{
        reader::{GpsReader, ParseOut},
        stack::GeoStack,
    },
};

use embassy_executor::Spawner;
use embassy_nrf::{
    bind_interrupts,
    gpio::{Input, Pull},
    peripherals,
    twim::{self, Twim},
    uarte::{self, Baudrate, Parity},
};
use nmea::sentences::FixType;

use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_10X20, iso_8859_15::FONT_5X8},
    pixelcolor::BinaryColor,
    prelude::*,
};

use ssd1306::{I2CDisplayInterface, Ssd1306, prelude::*};
mod draw_fns;
bind_interrupts!(struct Irqs {
    TWISPI0 => twim::InterruptHandler<peripherals::TWISPI0>;
    UARTE0 => uarte::InterruptHandler<peripherals::UARTE0>;
});

static CHANNEL: StaticCell<Channel<NoopRawMutex, ParseOut, 1>> = StaticCell::new();
static SHARED_STATE: StaticCell<Mutex<NoopRawMutex, SharedState>> = StaticCell::new();
static GPS_READER: StaticCell<GpsReader<'static>> = StaticCell::new();

const TEXT_STYLE_LG: MonoTextStyle<'_, BinaryColor> =
    MonoTextStyle::new(&FONT_10X20, BinaryColor::On);
const TEXT_STYLE_SM: MonoTextStyle<'_, BinaryColor> =
    MonoTextStyle::new(&FONT_5X8, BinaryColor::On);

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

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let shared = SHARED_STATE.init(Mutex::new(SharedState {
        is_recording: false,
    }));

    let p = embassy_nrf::init(Default::default());

    // set up uarte
    let mut uart_config = uarte::Config::default();
    uart_config.parity = Parity::EXCLUDED;
    uart_config.baudrate = Baudrate::BAUD115200;

    let uart = uarte::Uarte::new(p.UARTE0, Irqs, p.P0_08, p.P0_06, uart_config);

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

    let mut x = 128;

    let mut last_fix: Option<FixType> = None;

    let mut last_lat_lon_alt: Option<gps::reader::GpsReaderResults> = None;

    let mut geo_stack = GeoStack::new();

    spawner.spawn(button_task(button, shared)).unwrap();
    spawner.spawn(gps_reader_task(gps_reader)).unwrap();

    loop {
        Timer::after(Duration::from_millis(100)).await;
        display.clear(BinaryColor::Off).unwrap();

        draw_static_text(&mut display, TEXT_STYLE_LG).unwrap();
        draw_moving_jo(x, &mut display);

        while let Ok(gps_parse) = gps_channel.receiver().try_receive() {
            last_fix = gps_parse.fix.or(last_fix);
            let new_coords = gps_parse.lat_lon_altitude.or(last_lat_lon_alt);
            last_lat_lon_alt = new_coords;
            if let Some(coords) = new_coords {
                geo_stack.add_coords(coords);
            }
        }

        draw_coords(&last_lat_lon_alt, &mut display);
        draw_fix_status(last_fix, &mut display);

        let is_recording = {
            let lock = shared.lock().await;
            lock.is_recording
        };

        // draw_recording_status(is_recording, &mut display);
        draw_total_distance(geo_stack.total_distance, &mut display);
        draw_current_speed(geo_stack.current_speed_mph, &mut display);
        draw_last_segment_distance(geo_stack.last_segment_distance, &mut display);
        draw_hdop(geo_stack.current_hdop, &mut display);

        display.flush().unwrap();

        x = if x <= -24 { 128 } else { x - 1 };
    }
}
