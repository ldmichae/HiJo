use embassy_nrf::{gpio::{Level, Output}, peripherals::{P0_02, P1_11, P1_13, P1_15, SPI2}, spim::{self, Spim}};
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, mutex::Mutex};
use embassy_time::Delay;
use embedded_hal_bus::spi::{ExclusiveDevice};
use embedded_sdmmc::{SdCard, TimeSource, Timestamp, VolumeManager};

use crate::{Irqs};

#[derive(Default)]
pub struct DummyTimesource();
impl TimeSource for DummyTimesource {
    // In theory you could use the RTC of the rp2040 here, if you had
    // any external time synchronizing device.
    fn get_timestamp(&self) -> Timestamp {
        Timestamp {
            year_since_1970: 0,
            zero_indexed_month: 0,
            zero_indexed_day: 0,
            hours: 0,
            minutes: 0,
            seconds: 0,
        }
    }
}

pub struct SetupPins {
    pub spi2: SPI2,
    pub sck: P1_11,
    pub miso: P1_13,
    pub mosi: P1_15,
    pub output: P0_02,
}

pub type VolumeMgrConfigured = VolumeManager<SdCard<ExclusiveDevice<embassy_nrf::spim::Spim<'static, embassy_nrf::peripherals::SPI2>, Output<'static>, Delay>, Delay>, DummyTimesource>;
pub type ConfiguredSd = SdCard<ExclusiveDevice<Spim<'static, SPI2>, Output<'static>, Delay>, Delay>;
pub struct SdHardware {
    pub spi_dev: Mutex<NoopRawMutex, ExclusiveDevice<Spim<'static, SPI2>, Output<'static>, Delay>>,
}

pub fn setup_hardware(p: SetupPins) -> SdHardware {
    let mut spim_config = spim::Config::default();
    spim_config.mode = spim::MODE_0;
    spim_config.frequency = spim::Frequency::M8;

    let delay = Delay;

    let spi_bus = spim::Spim::new(p.spi2, Irqs, p.sck, p.miso, p.mosi, spim_config);
    let sd_cs = Output::new(p.output, Level::High, embassy_nrf::gpio::OutputDrive::Standard0HighDrive1);
    let spi_dev = ExclusiveDevice::new(spi_bus, sd_cs, delay);

    SdHardware { spi_dev: Mutex::new(spi_dev) }
}