use chrono::NaiveTime;
use defmt::info;
use embassy_nrf::uarte::UarteRxWithIdle;
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, channel::Sender};
use heapless::String;
use nmea::{Nmea, sentences::FixType};

pub struct GpsReader<'a> {
    rx: UarteRxWithIdle<'a>,
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
    pub timestamp: Option<NaiveTime>,
}
impl<'a> GpsReader<'a> {
    pub fn new(
        rx: UarteRxWithIdle<'a>,
        sender: Sender<'static, NoopRawMutex, ParseOut, 1>,
    ) -> Self {
        GpsReader {
            rx,
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
            timestamp: self.parser.fix_timestamp(),
        }
    }

    fn parse_line(&mut self, line: heapless::String<128>) -> Option<ParseOut> {
        let mut msg = heapless::String::<128>::new();
        info!("New line collected {:?}", line.as_str());
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

    async fn flush_buffer(&mut self) {
        if !self.sentence_buffer.is_empty() {
            let mut line = heapless::String::new();
            core::mem::swap(&mut line, &mut self.sentence_buffer);

            if let Some(out) = self.parse_line(line) {
                self.sender.send(out).await;
            }
            self.sentence_buffer.clear();
        }
    }

    /// This is the logic you had in your loop before,
    /// now extracted so it can handle bytes from a DMA chunk.
    async fn process_byte(&mut self, byte: u8) {
        match byte {
            b'$' => {
                // If we hit a '$' and the buffer isn't empty, it means we
                // likely missed a newline, so parse what we have first.
                self.flush_buffer().await;
                let _ = self.sentence_buffer.push('$');
            }
            b'\n' => {
                self.flush_buffer().await;
            }
            b'\r' => {} // Ignore
            _ => {
                // Only add to buffer if we've seen a '$' (buffer not empty)
                if !self.sentence_buffer.is_empty() {
                    if self.sentence_buffer.push(byte as char).is_err() {
                        // Buffer overflowed, clear it and wait for next '$'
                        self.sentence_buffer.clear();
                    }
                }
            }
        }
    }

    pub async fn run(&mut self) {
        // This buffer is in RAM, so EasyDMA is happy.
        let mut dma_buffer = [0u8; 512];

        loop {
            // Read until 64 bytes are full OR the line goes idle
            match self.rx.read_until_idle(&mut dma_buffer).await {
                Ok(n) if n > 0 => {
                    // We received 'n' bytes. Process them one by one.
                    for i in 0..n {
                        self.process_byte(dma_buffer[i]).await;
                    }
                }
                Ok(_) => {}
                Err(e) => info!("UART Error: {:?}", e),
            }
        }
    }
}
