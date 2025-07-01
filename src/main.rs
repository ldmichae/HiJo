#![no_std]
#![no_main]

mod float;
mod gps;
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, channel::Channel, mutex::Mutex};
use embassy_time::{Duration, Timer};
use panic_halt as _;
use static_cell::StaticCell;

use crate::{
    float::FloatToString,
    gps::{GpsReader, ParseOut},
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
    text::{Alignment, Text},
};

use ssd1306::{I2CDisplayInterface, Ssd1306, prelude::*};

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

fn draw_static_text<D>(display: &mut D, lg: MonoTextStyle<BinaryColor>) -> Result<(), D::Error>
where
    D: DrawTarget<Color = BinaryColor>,
{
    Text::with_alignment("HIJO", Point::new(64, 12), lg, Alignment::Center).draw(display)?;

    Ok(())
}

fn draw_optional_float<D>(
    display: &mut D,
    value: Option<impl Into<f64>>,
    y: i32,
    style: MonoTextStyle<BinaryColor>,
) where
    D: DrawTarget<Color = BinaryColor>,
{
    if let Some(v) = value {
        let mut float_buf = FloatToString::new();
        let text = float_buf.convert(v.into());
        let _ = Text::new(text, Point::new(0, y), style).draw(display);
    }
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

    let shared = SHARED_STATE.init(Mutex::new(SharedState {
        is_recording: false,
    }));

    let gps_reader = GPS_READER.init(GpsReader::new(uart, gps_channel.sender()));

    let button = Input::new(p.P0_17, Pull::Up);

    let mut x = 128;

    let mut last_fix: Option<FixType> = None;
    let mut last_lat_lon_alt: Option<gps::LatLonAlt> = None;

    spawner.spawn(button_task(button, shared)).unwrap();
    spawner.spawn(gps_reader_task(gps_reader)).unwrap();

    loop {
        Timer::after(Duration::from_millis(100)).await;
        display.clear(BinaryColor::Off).unwrap();

        draw_static_text(&mut display, TEXT_STYLE_LG).unwrap();

        Text::new("hijo", Point::new(x, 52), TEXT_STYLE_SM)
            .draw(&mut display)
            .unwrap();

        while let Ok(gps_parse) = gps_channel.receiver().try_receive() {
            last_fix = gps_parse.fix.or(last_fix);
            last_lat_lon_alt = gps_parse.lat_lon_altitude.or(last_lat_lon_alt);
        }

        if let Some(lat_lon_alt) = &last_lat_lon_alt {
            draw_optional_float(&mut display, lat_lon_alt.lat, 32, TEXT_STYLE_SM);
            draw_optional_float(&mut display, lat_lon_alt.lon, 40, TEXT_STYLE_SM);
            draw_optional_float(
                &mut display,
                lat_lon_alt.alt.map(f64::from),
                48,
                TEXT_STYLE_SM,
            );
        }

        if let Some(fix) = &last_fix {
            let fix_text = match fix {
                FixType::Invalid => "INVALID",
                FixType::Gps => "GPS",
                FixType::DGps => "DGPS",
                _ => "OTHER",
            };
            Text::new(fix_text, Point::new(4, 60), TEXT_STYLE_SM)
                .draw(&mut display)
                .unwrap();
        } else {
            Text::new("NO GPS", Point::new(4, 60), TEXT_STYLE_SM)
                .draw(&mut display)
                .unwrap();
        }

        let is_recording = {
            let lock = shared.lock().await;
            lock.is_recording
        };

        let recording_state_text = if is_recording { "STOP" } else { "START" };
        Text::new(recording_state_text, Point::new(72, 32), TEXT_STYLE_SM)
            .draw(&mut display)
            .unwrap();

        display.flush().unwrap();

        x = if x <= -24 { 128 } else { x - 1 };
    }
}
