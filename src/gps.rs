use heapless::String;
use nmea::{Nmea, sentences::FixType};

use crate::uart::GpsUart;

pub struct Gps {
    uart: GpsUart,
    rx_buffer: [u8; 1],
    sentence_buffer: heapless::String<128>, // or any fixed-size string buffer
    parser: Nmea,
}

pub struct ParseOut {
    pub fix: Option<FixType>,
    pub line: String<128>,
    pub lla: Option<LLA>,
}

pub struct LLA {
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub alt: Option<f32>,
}

impl Gps {
    pub fn init(uart: GpsUart) -> Self {
        Gps {
            uart,
            rx_buffer: [0; 1],
            sentence_buffer: heapless::String::new(),
            parser: Nmea::default(),
        }
    }

    pub fn read_and_parse(&mut self) -> Option<ParseOut> {
        for _ in 0..256 {
            if self.uart.read(&mut self.rx_buffer).is_ok() {
                let byte = self.rx_buffer[0];

                match byte {
                    b'$' => {
                        // If buffer not empty, parse existing sentence first
                        if !self.sentence_buffer.is_empty() {
                            let mut line = heapless::String::new();
                            core::mem::swap(&mut line, &mut self.sentence_buffer);
                            if let Some(out) = self.parse_line(line) {
                                return Some(out);
                            }
                        }
                        self.sentence_buffer.clear();
                        let _ = self.sentence_buffer.push('$');
                    }
                    // b'\n' => {
                    //     // End of sentence - parse if buffer not empty
                    //     if !self.sentence_buffer.is_empty() {
                    //         let mut line = heapless::String::new();
                    //         core::mem::swap(&mut line, &mut self.sentence_buffer);
                    //         if let Some(out) = self.parse_line(line) {
                    //             return Some(out);
                    //         }
                    //     }
                    // }
                    b'\r' => {
                        // Ignore carriage return
                    }
                    _ => {
                        // Only accumulate if sentence started with '$'
                        if !self.sentence_buffer.is_empty() {
                            let _ = self.sentence_buffer.push(byte as char);
                        }
                    }
                }
            } else {
                // UART read failed
                self.sentence_buffer.clear();
                let mut text: heapless::String<128> = heapless::String::new();
                let _ = text.push_str("NO UART");
                return Some(ParseOut {
                    fix: None,
                    line: text,
                    lla: None,
                });
            }
        }
        None
    }

    fn get_pos(&mut self) -> LLA {
        let lat = self.parser.latitude();
        let lon = self.parser.longitude();
        let alt = self.parser.altitude();

        return LLA { lat, lon, alt };
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
                            lla: Some(lla),
                        })
                    } else {
                        let _ = msg.push_str("[NOFIX] ");
                        let _ = msg.push_str(&line);
                        Some(ParseOut {
                            fix: None,
                            line: msg,
                            lla: None,
                        })
                    }
                }
                Err(_) => {
                    let _ = msg.push_str("[ERR] ");
                    let _ = msg.push_str(&line);
                    Some(ParseOut {
                        fix: None,
                        line: msg,
                        lla: None,
                    })
                }
            }
        } else {
            let _ = msg.push_str("[BAD] ");
            let _ = msg.push_str(&line);
            Some(ParseOut {
                fix: None,
                line: msg,
                lla: None,
            })
        }
    }
}
