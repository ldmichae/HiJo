use embassy_nrf::{gpio::{Level, Output}, peripherals::{P0_02, P1_11, P1_13, P1_15, SPI2}, spim::{self, Spim}};
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, mutex::Mutex};
use embassy_time::Delay;
use embedded_hal_bus::spi::ExclusiveDevice;
use embedded_sdmmc::{BlockDevice, Directory, Error, Mode, SdCard, TimeSource, Timestamp, VolumeIdx, VolumeManager};

use crate::{gps::reader::GpsReaderResults, Irqs};

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
pub type WriteableDirectory<'a> = Directory<'a, SdCard<ExclusiveDevice<Spim<'static, SPI2>, Output<'static>, Delay>, Delay>, DummyTimesource, 4, 4, 1>;

pub fn setup(p: SetupPins) -> VolumeMgrConfigured {
    // set up spi
    let mut spim_config = spim::Config::default();
    spim_config.mode = spim::MODE_0;
    spim_config.frequency = spim::Frequency::M8;

    let delay = Delay;

    let spi_bus = spim::Spim::new(p.spi2, Irqs, p.sck, p.miso, p.mosi, spim_config);
    let sd_cs = Output::new(p.output, Level::High, embassy_nrf::gpio::OutputDrive::Standard0HighDrive1);
    let spi_dev = ExclusiveDevice::new(spi_bus, sd_cs, delay);

    let sdcard = SdCard::new(spi_dev, Delay);
    let volume_mgr = VolumeManager::new(sdcard, DummyTimesource::default());

    return volume_mgr
}

pub fn write_coordinates<D: BlockDevice, T: TimeSource, const DIRS: usize, const FILES: usize, const VOLUMES: usize>(
    root_dir: &mut Directory<D, T, DIRS, FILES, VOLUMES>,
    coordinates: GpsReaderResults,
) -> Result<(), Error<D::Error>>
{
    let my_other_file = root_dir.open_file_in_dir("MY_DATA.CSV", Mode::ReadWriteCreateOrAppend)?;
    my_other_file.write(b"Timestamp,Signal,Value\n")?;
    my_other_file.write(b"2025-01-01T00:00:00Z,TEMP,25.0\n")?;
    my_other_file.write(b"2025-01-01T00:00:01Z,TEMP,25.1\n")?;
    my_other_file.write(b"2025-01-01T00:00:02Z,TEMP,25.2\n")?;
    // Don't forget to flush the file so that the directory entry is updated
    my_other_file.flush()?;
    Ok(())
}