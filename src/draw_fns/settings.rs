use embassy_nrf::twim::Twim;
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, mutex::Mutex};
use embedded_graphics::{prelude::*, text::Text};
use ssd1306::{
    Ssd1306, mode::BufferedGraphicsMode, prelude::I2CInterface, size::DisplaySize128x64,
};

use crate::{SettingsState, SettingsWrapper, TEXT_STYLE_SM};

pub async fn draw_settings(
    display: &mut Ssd1306<
        I2CInterface<Twim<'_>>,
        DisplaySize128x64,
        BufferedGraphicsMode<DisplaySize128x64>,
    >,
    settings_state: &Mutex<NoopRawMutex, SettingsState>,
) {
    let settings_state_lock = settings_state.lock().await;
    let items = &settings_state_lock.items;
    let cursor_pos = settings_state_lock.index;
    let get_cursor_y = || -> i32 {
        let preliminary_usize_offset = 30 + (10 * cursor_pos);
        preliminary_usize_offset.try_into().unwrap()
    };

    let cursor_point = Point::new(1, get_cursor_y());

    for (idx, item) in items.iter().enumerate() {
        let y_pos = 30 + (idx * 10) as i32;
        match item {
            SettingsWrapper::Default => {}
            SettingsWrapper::Bool(setting) => {
                Text::new(setting.label, Point::new(16, y_pos), TEXT_STYLE_SM)
                    .draw(display)
                    .unwrap();
                Text::new(
                    setting.options.current().0,
                    Point::new(90, y_pos),
                    TEXT_STYLE_SM,
                )
                .draw(display)
                .unwrap();
            }
            SettingsWrapper::Text(setting) => {
                Text::new(setting.label, Point::new(16, y_pos), TEXT_STYLE_SM)
                    .draw(display)
                    .unwrap();
                Text::new(
                    setting.options.current().0,
                    Point::new(90, y_pos),
                    TEXT_STYLE_SM,
                )
                .draw(display)
                .unwrap();
            }
            SettingsWrapper::AnyNumber(setting) => {
                Text::new(setting.label, Point::new(16, y_pos), TEXT_STYLE_SM)
                    .draw(display)
                    .unwrap();
                Text::new(
                    setting.options.current().0,
                    Point::new(90, y_pos),
                    TEXT_STYLE_SM,
                )
                .draw(display)
                .unwrap();
            }
        };
    }

    Text::new(">", cursor_point, TEXT_STYLE_SM)
        .draw(display)
        .unwrap();
}
