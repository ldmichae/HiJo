use chrono::{NaiveTime};
use embassy_nrf::{peripherals, uarte};
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, channel::Sender};
use heapless::{String, Vec};
use nmea::{Nmea, sentences::FixType};
use core::fmt::Write;

use crate::gps::assembler::NmeaAssembler;

pub struct GpsReader<'a> {
    uart: uarte::Uarte<'a, peripherals::SERIAL1>,
    rx_buffer: [u8; 1],
    parser: Nmea,
    sender: Sender<'static, NoopRawMutex, ParseOut, 1>,
    assembler: NmeaAssembler
}

pub struct ParseOut {
    pub fix: Option<FixType>,
    pub line: String<128>,
    // Optional: parsed lat/lon/alt
    pub reader_results: Option<GpsReaderResults>,
    pub reader_datetime: Option<InternalDateDTO>,
}

#[derive(Copy, Clone)]
pub struct GpsReaderResults {
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub alt: Option<f32>,
    pub hdop: Option<f32>,
    pub timestamp: Option<NaiveTime>,
}

#[derive(Clone)]
pub struct InternalDateDTO {
    pub day: u8,
    pub month: u8,
    pub year: u32,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub pretty_date: String<10>,
    pub pretty_time: String<5>,
}

impl<'a> GpsReader<'a> {
    pub fn new(
        uart: uarte::Uarte<'a, peripherals::SERIAL1>,
        sender: Sender<'static, NoopRawMutex, ParseOut, 1>,
    ) -> Self {
        GpsReader {
            uart,
            rx_buffer: [0; 1],
            parser: Nmea::default(),
            sender,
            assembler: NmeaAssembler::new()
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

    fn get_date_time(&mut self, line: &str) -> InternalDateDTO {
        // SAMPLE ZDA
        // "$GNZDA,195027.000,07,09,2025,,*4B"
        let items: Vec<_, 10> = line.split(",").collect();
        let hhmmss = items[1];
        let day = items[2].parse().unwrap();
        let month = items[3].parse().unwrap();
        let year = items[4].parse().unwrap();
        let hour = hhmmss.get(0..2).unwrap().parse().unwrap();
        let minute = hhmmss.get(2..4).unwrap().parse().unwrap();
        let second = hhmmss.get(4..6).unwrap().parse().unwrap();

        let mut pretty_date = heapless::String::<10>::new();
        let _ = write!(pretty_date, "{:02}/{:02}/{:02}", day, month, items[4].get(2..4).unwrap());

        let mut pretty_time = heapless::String::<5>::new();
        let _ = write!(pretty_time, "{:02}:{:02}", hour, minute);

        InternalDateDTO { day, month, year, hour, minute, second, pretty_date, pretty_time }
    }

    fn is_sentence_of_interest(&mut self, line: &heapless::String<82>) -> bool {
        line.starts_with("$GNGGA") || line.starts_with("$GNZDA")
    }

    fn parse_line(&mut self, line: &heapless::String<82>) -> Option<ParseOut> {
        let mut msg = heapless::String::<128>::new();
        if line.starts_with("$GNGGA"){
            match self.parser.parse(&line) {
                Ok(_) => {
                    if let Some(fix) = self.parser.fix_type() {
                        let _ = msg.push_str("[GGA-OK] ");
                        let _ = msg.push_str(&line);
                        let lla = self.get_pos();
                        Some(ParseOut {
                            fix: Some(fix),
                            line: msg,
                            reader_results: Some(lla),
                            reader_datetime: None,
                        })
                    } else {
                        let _ = msg.push_str("[GGA-NOFIX] ");
                        let _ = msg.push_str(&line);
                        Some(ParseOut {
                            fix: None,
                            line: msg,
                            reader_results: None,
                            reader_datetime: None,
                        })
                    }
                }
                Err(_) => {
                    let mut msg = heapless::String::<128>::new();
                    let _ = msg.push_str("[ERR] ");
                    let _ = msg.push_str(&line);
                    Some(ParseOut {
                        fix: None,
                        line: msg,
                        reader_results: None,
                        reader_datetime: None,
                    })
                }
            }
        } else {
            defmt::info!("parsing zda, i think... {:?}", line.as_str());
            // GNZDA: just timestamp info
            let _ = msg.push_str("[ZDA] ");
            let _ = msg.push_str(&line);
            let dt = self.get_date_time(line.as_str());
            Some(ParseOut {
                fix: None,
                line: msg,
                reader_results: None,
                reader_datetime: Some(dt),
            })
        }
    }


    pub async fn run(&mut self) {
        loop {
            if self.uart.read(&mut self.rx_buffer).await.is_ok() {
                let byte = self.rx_buffer[0];

                if let Some(sentence) = self.assembler.push_byte(byte) {
                    if self.is_sentence_of_interest(&sentence){
                        defmt::info!("got full sentence: {}", sentence.as_str());
                        if let Some(out) = self.parse_line(&sentence) {
                            self.sender.send(out).await;
                        } else {
                            defmt::info!("parse failed: {}", sentence.as_str());
                        }
                    }
                }
            }
        }
    }
}