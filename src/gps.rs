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
    pub lat_lon_altitude: Option<LatLonAlt>,
}

pub struct LatLonAlt {
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
        loop {
            match self.uart.read(&mut self.rx_buffer) {
                Ok(()) => {
                    let byte = self.rx_buffer[0];
                    match byte {
                        b'$' => {
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
                        b'\r' => {}
                        _ => {
                            if !self.sentence_buffer.is_empty() {
                                let _ = self.sentence_buffer.push(byte as char);
                            }
                        }
                    }
                }
                Err(_) => break, // No more data available, exit loop
            }
        }

        None
    }

    fn get_pos(&mut self) -> LatLonAlt {
        let lat = self.parser.latitude();
        let lon = self.parser.longitude();
        let alt = self.parser.altitude();

        return LatLonAlt { lat, lon, alt };
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
                            lat_lon_altitude: Some(lla),
                        })
                    } else {
                        let _ = msg.push_str("[NOFIX] ");
                        let _ = msg.push_str(&line);
                        Some(ParseOut {
                            fix: None,
                            line: msg,
                            lat_lon_altitude: None,
                        })
                    }
                }
                Err(_) => {
                    let _ = msg.push_str("[ERR] ");
                    let _ = msg.push_str(&line);
                    Some(ParseOut {
                        fix: None,
                        line: msg,
                        lat_lon_altitude: None,
                    })
                }
            }
        } else {
            let _ = msg.push_str("[BAD] ");
            let _ = msg.push_str(&line);
            Some(ParseOut {
                fix: None,
                line: msg,
                lat_lon_altitude: None,
            })
        }
    }
}
