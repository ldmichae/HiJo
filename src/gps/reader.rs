use embassy_nrf::{peripherals, uarte};
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, channel::Sender};
use heapless::String;
use nmea::{Nmea, sentences::FixType};

pub struct GpsReader<'a> {
    uart: uarte::Uarte<'a, peripherals::UARTE0>,
    rx_buffer: [u8; 1],
    sentence_buffer: heapless::String<128>,
    parser: Nmea,
    sender: Sender<'static, NoopRawMutex, ParseOut, 1>,
}

pub struct ParseOut {
    pub fix: Option<FixType>,
    pub line: String<128>,
    // Optional: parsed lat/lon/alt
    pub reader_results: Option<GpsReaderResults>,
}

#[derive(Copy, Clone)]
pub struct GpsReaderResults {
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub alt: Option<f32>,
    pub hdop: Option<f32>,
    pub timestamp: Option<u64>,
}
impl<'a> GpsReader<'a> {
    pub fn new(
        uart: uarte::Uarte<'a, peripherals::UARTE0>,
        sender: Sender<'static, NoopRawMutex, ParseOut, 1>,
    ) -> Self {
        GpsReader {
            uart,
            rx_buffer: [0; 1],
            sentence_buffer: heapless::String::new(),
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
            timestamp: self.parser.fix_timestamp()
        }
    }

    fn parse_line(&mut self, line: heapless::String<128>) -> Option<ParseOut> {
        let mut msg = heapless::String::<128>::new();

        if line.starts_with('$') && line.contains('*') {
            match self.parser.parse(&line) {
                Ok(_) => {
                    if let Some(fix) = self.parser.fix_type() {
                        let _ = msg.push_str("[OK] ");
                        let _ = msg.push_str(&line);
                        let lla = self.get_pos();
                        Some(ParseOut {
                            fix: Some(fix),
                            line: msg,
                            reader_results: Some(lla),
                        })
                    } else {
                        let _ = msg.push_str("[NOFIX] ");
                        let _ = msg.push_str(&line);
                        Some(ParseOut {
                            fix: None,
                            line: msg,
                            reader_results: None,
                        })
                    }
                }
                Err(_) => {
                    let _ = msg.push_str("[ERR] ");
                    let _ = msg.push_str(&line);
                    Some(ParseOut {
                        fix: None,
                        line: msg,
                        reader_results: None,
                    })
                }
            }
        } else {
            let _ = msg.push_str("[BAD] ");
            let _ = msg.push_str(&line);
            Some(ParseOut {
                fix: None,
                line: msg,
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
                        // Start of new sentence
                        if !self.sentence_buffer.is_empty() {
                            // Parse previous sentence before starting new one
                            let mut line = heapless::String::new();
                            core::mem::swap(&mut line, &mut self.sentence_buffer);
                            if let Some(out) = self.parse_line(line) {
                                self.sender.send(out).await;
                            }
                            self.sentence_buffer.clear();
                        }
                        let _ = self.sentence_buffer.push('$');
                    }
                    b'\n' => {
                        // End of sentence — parse what we have
                        if !self.sentence_buffer.is_empty() {
                            let mut line = heapless::String::new();
                            core::mem::swap(&mut line, &mut self.sentence_buffer);
                            if let Some(out) = self.parse_line(line) {
                                self.sender.send(out).await;
                            }
                            self.sentence_buffer.clear();
                        }
                    }
                    b'\r' => {
                        // Just ignore carriage return
                    }
                    _ => {
                        if !self.sentence_buffer.is_empty() {
                            let _ = self.sentence_buffer.push(byte as char);
                        }
                    }
                }
            }
        }
    }
}
