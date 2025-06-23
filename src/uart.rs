use hal::gpio::{Disconnected, Level};
use hal::pac::UARTE0;
use hal::uarte::{Baudrate, Parity, Pins, Uarte};
use nrf52840_hal as hal;
use nrf52840_hal::gpio::p0::{P0_06, P0_08};

pub type GpsUart = Uarte<UARTE0>;

pub fn init_uart(
    tx_pin: P0_06<Disconnected>,
    rx_pin: P0_08<Disconnected>,
    uarte0: UARTE0,
) -> GpsUart {
    let tx = tx_pin.into_push_pull_output(Level::Low).degrade();
    let rx = rx_pin.into_floating_input().degrade();

    let pins = Pins {
        txd: tx,
        rxd: rx,
        cts: None,
        rts: None,
    };

    Uarte::new(uarte0, pins, Parity::EXCLUDED, Baudrate::BAUD115200)
}
