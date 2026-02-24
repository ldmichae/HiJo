#![no_main]
#![no_std]

mod gps;
mod utils;
use defmt::*;
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, channel::Channel, mutex::Mutex};
use embassy_time::{Duration, Timer};
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

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
    bind_interrupts,
    gpio::{Input, Level, Output, OutputDrive, Pull},
    peripherals,
    twim::{self, Twim},
    uarte::{self, Baudrate, Parity, UarteTx},
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
    SERIAL1 => uarte::InterruptHandler<peripherals::SERIAL1>;
});

static CHANNEL: StaticCell<Channel<NoopRawMutex, ParseOut, 1>> = StaticCell::new();
static SHARED_STATE: StaticCell<Mutex<NoopRawMutex, SharedState>> = StaticCell::new();
static BLINK_STATE: StaticCell<Mutex<NoopRawMutex, bool>> = StaticCell::new();

static I2C_BUFFER: StaticCell<[u8; 1024]> = StaticCell::new();

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
    info!("Running gps reader task");
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
                info!("Button press confirmed");
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

async fn disable_unwanted_sentences(tx: &mut UarteTx<'_>) {
    let commands: &[&[u8]] = &[
        b"$PAIR062,2,0*3c\r\n",
        b"$PAIR062,3,0*3d\r\n",
        b"$PAIR062,4,0*3a\r\n",
        b"$PAIR062,5,0*3b\r\n",
        b"$PAIR062,6,0*38\r\n",
        b"$PAIR062,7,0*39\r\n",
        b"$PAIR062,8,0*36\r\n",
        b"$PAIR062,9,0*37\r\n",
        b"$PAIR062,1,10*0e\r\n",
    ];
    for cmd in commands {
        let _ = tx.write(cmd).await;
        // Give the GPS chip a breath to parse the NMEA sentence
        Timer::after(Duration::from_millis(100)).await;
    }
    let _ = tx.write(b"$PAIR513*3D\r\n").await;
    info!("GPS Configuration commands sent.");
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("Starting.");
    let shared = SHARED_STATE.init(Mutex::new(SharedState {
        is_recording: false,
    }));
    let blink_mutex_ref = BLINK_STATE.init(Mutex::new(true));
    info!("Configuring Embassy");
    let mut config = embassy_nrf::config::Config::default();
    config.hfclk_source = embassy_nrf::config::HfclkSource::Internal;
    config.lfclk_source = embassy_nrf::config::LfclkSource::InternalRC;
    let p = embassy_nrf::init(config);
    info!("Embassy Configured");
    // set up uarte
    info!("setting up uarte");
    let mut uart_config = uarte::Config::default();
    uart_config.parity = Parity::EXCLUDED;
    uart_config.baudrate = Baudrate::BAUD115200;
    info!("initializing uarte");
    let uart = uarte::Uarte::new(p.SERIAL1, p.P1_10, p.P1_11, Irqs, uart_config);

    // set up display
    info!("Configuring I2C...");
    let twim_config = twim::Config::default();
    let ibuf = I2C_BUFFER.init([0u8; 1024]);
    let i2c = Twim::new(p.SERIAL0, Irqs, p.P1_12, p.P1_14, twim_config, ibuf);
    info!("initializing interface i2c");
    let interface = I2CDisplayInterface::new(i2c);
    info!("creating display");
    let mut display = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
    .into_buffered_graphics_mode();

    info!("Calling display.init()...");
    // REMOVE the .unwrap() line! Just use this:
    match display.init() {
        Ok(_) => info!("Display Success!"),
        Err(_) => info!("Display Init Error"),
    }

    info!("Moving on to GPS setup...");
    let gps_channel = CHANNEL.init(Channel::new());
    // Power cycle the unit
    let mut gps_power = Output::new(p.P1_15, Level::Low, OutputDrive::HighDrive);
    Timer::after(Duration::from_millis(1000)).await;
    gps_power.set_high();
    info!("Waiting for GPS to wake up...");
    let (mut tx, rx) = uart.split_with_idle(p.TIMER0, p.PPI_CH0, p.PPI_CH1);
    Timer::after(Duration::from_secs(2)).await; // Give it time to boot
    let gps_raw = GpsReader::new(rx, gps_channel.sender());
    info!("Configuring GPS module to not send unwanted sentences");
    disable_unwanted_sentences(&mut tx).await;
    Timer::after(Duration::from_millis(200)).await; // Let it settle
    let gps_reader = GPS_READER.init(gps_raw);

    let button = Input::new(p.P0_23, Pull::Up);

    let mut last_fix: Option<FixType> = None;

    let mut last_lat_lon_alt: Option<gps::reader::GpsReaderResults> = None;

    let mut geo_stack = GeoStack::new();

    spawner.spawn(button_task(button, shared)).unwrap();
    spawner.spawn(gps_reader_task(gps_reader)).unwrap();
    spawner
        .spawn(show_jo_updater_task(blink_mutex_ref))
        .unwrap();

    loop {
        let is_recording = {
            let lock = shared.lock().await;
            lock.is_recording
        };

        let should_blink = {
            let lock = blink_mutex_ref.lock().await;
            *lock
        };

        Timer::after(Duration::from_millis(100)).await;
        display.clear(BinaryColor::Off).unwrap();

        draw_static_text(&mut display, TEXT_STYLE_LG).unwrap();
        if should_blink {
            draw_blinky(&mut display);
        }

        while let Ok(gps_parse) = gps_channel.receiver().try_receive() {
            last_fix = gps_parse.fix.or(last_fix);
            let new_coords = gps_parse.reader_results.or(last_lat_lon_alt);
            last_lat_lon_alt = new_coords;
            if let Some(coords) = new_coords {
                geo_stack.add_coords(coords, is_recording);
            }
        }

        draw_coords(&last_lat_lon_alt, &mut display);

        draw_recording_status(is_recording, &mut display);
        draw_total_elev_gain(geo_stack.total_elevation_gain.into(), &mut display);
        draw_total_distance(geo_stack.total_distance, &mut display);
        draw_current_speed(geo_stack.current_speed_mph, &mut display);
        draw_hdop(last_fix, geo_stack.current_hdop, &mut display);

        display.flush().unwrap();
    }
}
