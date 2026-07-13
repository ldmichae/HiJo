use chrono::NaiveTime;
use defmt::info;
use embassy_nrf::buffered_uarte::BufferedUarte;
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, channel::Sender};
use heapless::Vec;
use nmea::{Nmea, sentences::FixType};

pub struct GpsReader<'a> {
    uart: BufferedUarte<'a>,
    rx_buffer: [u8; 1],
    sentence_buffer: Vec<u8, 82>,
    parser: Nmea,
    sender: Sender<'static, NoopRawMutex, ParseOut, 1>,
}

pub struct ParseOut {
    pub fix: Option<FixType>,
    // Optional: parsed lat/lon/alt
    pub reader_results: Option<GpsReaderResults>,
}

#[derive(Copy, Clone)]
pub struct GpsReaderResults {
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub alt: Option<f32>,
    pub hdop: Option<f32>,
    pub timestamp: Option<NaiveTime>,
}
impl<'a> GpsReader<'a> {
    pub fn new(
        uart: BufferedUarte<'a>,
        sender: Sender<'static, NoopRawMutex, ParseOut, 1>,
    ) -> Self {
        GpsReader {
            uart,
            rx_buffer: [0; 1],
            sentence_buffer: Vec::<u8, 82>::new(),
            parser: Nmea::default(),
            sender,
        }
    }

    fn get_pos(&mut self) -> GpsReaderResults {
        GpsReaderResults {
            lat: self.parser.latitude(),
            lon: self.parser.longitude(),
            alt: self.parser.altitude(),
            hdop: self.parser.hdop(),
            timestamp: self.parser.fix_timestamp(),
        }
    }

    fn parse_line(&mut self, line: Vec<u8, 82>) -> Option<ParseOut> {
        let msg = heapless::String::<82>::from(line.iter().map(|x| *x as char).collect());
        info!("MSG: {:?}", defmt::Debug2Format(&msg));
        if line.starts_with(&[b'$']) && line.contains(&b'*') {
            match self.parser.parse(&msg) {
                Ok(_) => {
                    if let Some(fix) = self.parser.fix_type() {
                        let lla = self.get_pos();
                        Some(ParseOut {
                            fix: Some(fix),
                            reader_results: Some(lla),
                        })
                    } else {
                        Some(ParseOut {
                            fix: None,
                            reader_results: None,
                        })
                    }
                }
                Err(_) => Some(ParseOut {
                    fix: None,
                    reader_results: None,
                }),
            }
        } else {
            Some(ParseOut {
                fix: None,
                reader_results: None,
            })
        }
    }

    pub async fn run(&mut self) {
        loop {
            if self.uart.read(&mut self.rx_buffer).await.is_ok() {
                let byte = self.rx_buffer[0];
                match byte {
                    b'$' => {
                        self.sentence_buffer.clear();
                        let _ = self.sentence_buffer.push(byte);
                    }
                    b'\n' => {
                        let line = core::mem::take(&mut self.sentence_buffer);
                        if let Some(out) = self.parse_line(line) {
                            self.sender.send(out).await;
                        }
                    }
                    b'\r' => {}
                    _ => {
                        if self.sentence_buffer.starts_with(b"$") {
                            let _ = self.sentence_buffer.push(byte);
                        }
                    }
                }
            }
        }
    }
}
