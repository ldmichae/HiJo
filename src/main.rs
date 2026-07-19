#![no_std]
#![no_main]

mod gps;
mod utils;

use defmt::info;
use embassy_embedded_hal::adapter::BlockingAsync;
use embassy_futures::select::{Either, select};
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, channel::Channel, mutex::Mutex};
use embassy_time::{Duration, Timer};
use sequential_storage::{
    cache::{Cache, Uncached},
    map::{MapConfig, MapStorage},
};
use static_cell::StaticCell;

use defmt_rtt as _;
use panic_probe as _;

use crate::{
    Page::{RECORD, SETTINGS},
    draw_fns::{
        settings::draw_settings,
        utils::{
            draw_blinky, draw_coords, draw_current_speed, draw_hdop, draw_recording_status,
            draw_static_text, draw_total_distance, draw_total_elev_gain,
        },
    },
    gps::{
        reader::{GpsReader, ParseOut},
        stack::GeoStack,
    },
    utils::vector::CircularTracker,
};

use embassy_executor::Spawner;
use embassy_nrf::{
    bind_interrupts,
    buffered_uarte::{self, BufferedUarte},
    gpio::{Input, Pull},
    nvmc::Nvmc,
    peripherals::{self},
    twim::{self, Twim},
    uarte::{self, Baudrate, Parity},
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
static SETTINGS_STATE: StaticCell<Mutex<NoopRawMutex, SettingsState>> = StaticCell::new();
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Page {
    RECORD,
    SETTINGS,
}

pub struct SharedState {
    pub is_recording: bool,
    pub page: Page,
}

#[derive(Copy, Clone, Debug)]
pub struct Setting<T: Copy + Default> {
    pub id: &'static u8,
    pub label: &'static str,
    pub options: CircularTracker<8, (&'static str, T)>,
}

#[derive(Copy, Clone, Default, Debug)]
pub enum SettingsWrapper {
    #[default]
    Default,
    Bool(Setting<bool>),
    Text(Setting<&'static str>),
    AnyNumber(Setting<isize>),
}

type SettingsState = CircularTracker<3, SettingsWrapper>;

#[embassy_executor::task]
async fn gps_reader_task(gps_reader: &'static mut GpsReader<'static>) {
    gps_reader.run().await;
}

#[embassy_executor::task]
async fn action_button_task(
    button: Input<'static>,
    shared_state: &'static Mutex<NoopRawMutex, SharedState>,
    settings_state: &'static Mutex<NoopRawMutex, SettingsState>,
    mut flash_storage: MapStorage<
        u8,
        BlockingAsync<Nvmc<'static>>,
        Cache<Uncached, Uncached, Uncached, u8>,
    >,
) {
    let mut last_state = button.is_high(); // true if not pressed

    loop {
        let current_state = button.is_high(); // still true if not pressed

        if last_state && !current_state {
            // falling edge: just pressed
            // debounce
            Timer::after(Duration::from_millis(20)).await;

            if button.is_low() {
                // confirmed press
                let mut lock = shared_state.lock().await;
                if lock.page == SETTINGS {
                    let mut settings_lock = settings_state.lock().await;
                    let index = settings_lock.index;
                    let mut buf = [0u8; 32];
                    match &mut settings_lock.items[index] {
                        SettingsWrapper::Default => {}
                        SettingsWrapper::Bool(setting) => {
                            setting.options.next();
                            let v = &u8::try_from(setting.options.index).unwrap();
                            info!(
                                "Attempting to set flash setting for ID {}... idx_value {}... conv {}",
                                setting.id, setting.options.index, v
                            );
                            let res = flash_storage.store_item(&mut buf, setting.id, v).await;

                            info!("{:?}", res);
                        }
                        SettingsWrapper::Text(setting) => {
                            setting.options.next();
                            info!(
                                "Attempting to set flash setting for ID {}... idx_value {}",
                                setting.id, setting.options.index
                            );
                            let res = flash_storage
                                .store_item(
                                    &mut buf,
                                    setting.id,
                                    &u8::try_from(setting.options.index).unwrap(),
                                )
                                .await;
                            info!("{:?}", res);
                        }
                        SettingsWrapper::AnyNumber(setting) => {
                            setting.options.next();
                            info!(
                                "Attempting to set flash setting for ID {}... idx_value {}",
                                setting.id, setting.options.index
                            );
                            let res = flash_storage
                                .store_item(
                                    &mut buf,
                                    setting.id,
                                    &u8::try_from(setting.options.index).unwrap(),
                                )
                                .await;
                            info!("{:?}", res);
                        }
                    };
                } else {
                    lock.is_recording = !lock.is_recording;
                };
            }
        }

        last_state = current_state;
        Timer::after(Duration::from_millis(10)).await;
    }
}

#[embassy_executor::task]
async fn page_button_task(
    button: Input<'static>,
    shared: &'static Mutex<NoopRawMutex, SharedState>,
) {
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
                let next_page = if lock.page == SETTINGS {
                    RECORD
                } else {
                    SETTINGS
                };
                lock.page = next_page;
            }
        }

        last_state = current_state;
        Timer::after(Duration::from_millis(10)).await;
    }
}

#[embassy_executor::task]
async fn cursor_up_task(
    button: Input<'static>,
    shared_state: &'static Mutex<NoopRawMutex, SharedState>,
    settings_state: &'static Mutex<NoopRawMutex, SettingsState>,
) {
    let mut last_state = button.is_high(); // true if not pressed

    loop {
        let current_state = button.is_high(); // still true if not pressed

        if last_state && !current_state {
            // falling edge: just pressed
            // debounce
            Timer::after(Duration::from_millis(20)).await;

            if button.is_low() {
                let lock = shared_state.lock().await;
                if lock.page == SETTINGS {
                    // confirmed press
                    let mut lock = settings_state.lock().await;
                    lock.previous();
                }
            }
        }

        last_state = current_state;
        Timer::after(Duration::from_millis(10)).await;
    }
}

#[embassy_executor::task]
async fn cursor_down_task(
    button: Input<'static>,
    shared_state: &'static Mutex<NoopRawMutex, SharedState>,
    settings_state: &'static Mutex<NoopRawMutex, SettingsState>,
) {
    let mut last_state = button.is_high(); // true if not pressed

    loop {
        let current_state = button.is_high(); // still true if not pressed

        if last_state && !current_state {
            // falling edge: just pressed
            // debounce
            Timer::after(Duration::from_millis(20)).await;

            if button.is_low() {
                let lock = shared_state.lock().await;
                if lock.page == SETTINGS {
                    // confirmed press
                    let mut lock = settings_state.lock().await;
                    lock.next();
                }
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
    let blink_mutex_ref = BLINK_STATE.init(Mutex::new(true));

    let p = embassy_nrf::init(Default::default());

    let nvmc_blocking = Nvmc::new(p.NVMC);
    let flash = BlockingAsync::new(nvmc_blocking);
    let flash_addressing = 0x000E_0000..0x0010_0000;
    let map_config = MapConfig::new(flash_addressing);
    let mut storage = MapStorage::<u8, _, _>::new(flash, map_config, Cache::new_uncached());

    Timer::after_millis(250).await;
    let mut auto_pause_saved_val_buf = [0; 32];
    let auto_pause_saved_val = storage.fetch_item(&mut auto_pause_saved_val_buf, &1).await;
    let auto_pause_setting = SettingsWrapper::Bool(Setting {
        id: &1,
        label: "Auto Pause",
        options: CircularTracker::new(
            &[("Y", true), ("N", false)],
            auto_pause_saved_val.unwrap_or(None),
        ),
    });

    let mut time_zone_saved_val_buf = [0; 32];
    let time_zone_saved_val = storage.fetch_item(&mut time_zone_saved_val_buf, &2).await;
    let time_zone_setting = SettingsWrapper::AnyNumber(Setting {
        id: &2,
        label: "Time Zone",
        options: CircularTracker::new(
            &[("EST", -5), ("CST", -7), ("PST", -9)],
            time_zone_saved_val.unwrap_or(None),
        ),
    });

    let mut unit_saved_val_buf = [0; 32];
    let unit_saved_val = storage.fetch_item(&mut unit_saved_val_buf, &3).await;
    let unit_setting = SettingsWrapper::AnyNumber(Setting {
        id: &3,
        label: "Units",
        options: CircularTracker::new(&[("ft/mi", 0), ("m/km", 1)], unit_saved_val.unwrap_or(None)),
    });

    let shared_state = SHARED_STATE.init(Mutex::new(SharedState {
        is_recording: false,
        page: RECORD,
    }));

    let settings_vec = &[auto_pause_setting, time_zone_setting, unit_setting];

    let settings_state = SETTINGS_STATE.init(Mutex::new(CircularTracker::new(settings_vec, None)));

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
        p.P1_09,
        p.P0_06,
        Irqs,
        uart_config,
        &mut rx_buffer[..],
        &mut tx_buffer[..],
    );

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
    Timer::after_secs(1).await;
    display.init().unwrap();

    let gps_channel = CHANNEL.init(Channel::new());
    let gps_receiver = gps_channel.receiver();
    let gps_reader = GPS_READER.init(GpsReader::new(uart, gps_channel.sender()));

    let record_button = Input::new(p.P0_23, Pull::Up);
    let page_button = Input::new(p.P0_08, Pull::Up);
    let cursor_up_button = Input::new(p.P0_24, Pull::Up);
    let cursor_down_button = Input::new(p.P0_09, Pull::Up);

    let mut last_fix: Option<FixType> = None;

    let mut last_lat_lon_alt: Option<gps::reader::GpsReaderResults> = None;

    let mut geo_stack = GeoStack::new();

    spawner.spawn(page_button_task(page_button, shared_state).unwrap());
    spawner
        .spawn(action_button_task(record_button, shared_state, settings_state, storage).unwrap());
    spawner.spawn(cursor_up_task(cursor_up_button, shared_state, settings_state).unwrap());
    spawner.spawn(cursor_down_task(cursor_down_button, shared_state, settings_state).unwrap());
    spawner.spawn(gps_reader_task(gps_reader).unwrap());
    spawner.spawn(show_jo_updater_task(blink_mutex_ref).unwrap());

    loop {
        let (is_recording, page) = {
            let lock = shared_state.lock().await;
            (lock.is_recording, lock.page)
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

                if page == RECORD {
                    if should_blink {
                        draw_blinky(&mut display);
                    }
                    draw_coords(&last_lat_lon_alt, &mut display);

                    draw_recording_status(is_recording, &mut display);
                    draw_total_elev_gain(geo_stack.total_elevation_gain.into(), &mut display);
                    draw_total_distance(geo_stack.total_distance, &mut display);
                    draw_current_speed(geo_stack.current_speed_mph, &mut display);
                    draw_hdop(last_fix, geo_stack.current_hdop, &mut display);
                } else if page == SETTINGS {
                    draw_settings(&mut display, settings_state).await;
                }

                display.flush().unwrap();
            }
            Either::Second(gps_parse) => {
                last_fix = gps_parse.fix.or(last_fix);
                let new_coords = gps_parse.reader_results;
                if let Some(coords) = new_coords {
                    last_lat_lon_alt = new_coords;
                    geo_stack.add_coords(coords, last_lat_lon_alt, is_recording);
                }
            }
        }
    }
}
