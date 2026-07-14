use embassy_nrf::twim::Twim;
use embedded_graphics::{
    prelude::*,
    text::{Text},
};
use ssd1306::{
    Ssd1306, mode::BufferedGraphicsMode, prelude::I2CInterface, size::DisplaySize128x64,
};
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, mutex::Mutex};


use crate::{SettingsState, TEXT_STYLE_SM};

pub async fn draw_settings(
    display: &mut Ssd1306<
        I2CInterface<Twim<'_>>,
        DisplaySize128x64,
        BufferedGraphicsMode<DisplaySize128x64>,
    >,
    settings_state: &Mutex<NoopRawMutex, SettingsState>
) {
    let settings_state_lock = settings_state.lock().await;
    let items= &settings_state_lock.items;
    let cursor_pos = settings_state_lock.cursor_pos;
    let get_cursor_y = || -> i32 {
        let preliminary_usize_offset = 30 + (10 * cursor_pos);
        preliminary_usize_offset.try_into().unwrap()
    };

    let cursor_point = Point::new(1, get_cursor_y());

    Text::new(items.get(0).unwrap(), Point::new(16, 30), TEXT_STYLE_SM)
        .draw(display)
        .unwrap();

    Text::new(items.get(1).unwrap(), Point::new(16, 40), TEXT_STYLE_SM)
        .draw(display)
        .unwrap();

    Text::new(">", cursor_point, TEXT_STYLE_SM)
        .draw(display)
        .unwrap();





}