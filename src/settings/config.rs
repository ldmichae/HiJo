use embassy_embedded_hal::adapter::BlockingAsync;
use embassy_nrf::nvmc::Nvmc;
use sequential_storage::{
    cache::{Cache, Uncached},
    map::MapStorage,
};

use crate::{
    settings::settings::{Setting, SettingsWrapper},
    utils::vector::CircularTracker,
};

pub type ProjNVMCStorage =
    MapStorage<u8, BlockingAsync<Nvmc<'static>>, Cache<Uncached, Uncached, Uncached, u8>>;

macro_rules! setting {
    ($storage: expr, $variant: ident, $id: literal, $label: literal, $options: expr) => {{
        let mut buf = [0; 32];
        let saved_val = $storage.fetch_item(&mut buf, &$id).await;
        SettingsWrapper::$variant(Setting {
            id: &$id,
            label: $label,
            options: CircularTracker::new(&$options, saved_val.unwrap_or(None)),
        })
    }};
}

pub async fn configure_auto_pause_setting(storage: &mut ProjNVMCStorage) -> SettingsWrapper {
    setting!(storage, Bool, 1, "Auto Pause", [("Y", true), ("N", false)])
}

pub async fn configure_time_zone_setting(storage: &mut ProjNVMCStorage) -> SettingsWrapper {
    setting!(
        storage,
        AnyNumber,
        2,
        "Time Zone",
        [("EST", -5), ("CST", -7), ("PST", -9)]
    )
}

pub async fn configure_units_setting(storage: &mut ProjNVMCStorage) -> SettingsWrapper {
    setting!(storage, AnyNumber, 3, "Units", [("ft/mi", 0), ("m/km", 1)])
}
