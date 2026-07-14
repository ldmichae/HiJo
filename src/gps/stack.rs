use chrono::Duration;
use heapless::Deque;

use crate::gps::{
    fns::{LatLonAlt, calculate_speed, haversine_distance_ft},
    reader::GpsReaderResults,
};

const MAX_ITEMS: usize = 16;

pub struct GeoStack {
    pub stack: Deque<GpsReaderResults, MAX_ITEMS>,
    pub last_segment_distance: f64,
    pub total_distance: f64,
    pub total_elevation_gain: f32,
    pub current_speed_mph: f64,
    pub current_hdop: f32,
    pub min_time_interval_ms: i64,
    pub min_distance_threshold: f64,
}

impl GeoStack {
    pub fn new() -> Self {
        GeoStack {
            stack: Deque::new(),
            last_segment_distance: 0.0,
            total_distance: 0.0,
            total_elevation_gain: 0.0,
            current_speed_mph: 0.0,
            current_hdop: 0.0,
            min_time_interval_ms: 1000,
            min_distance_threshold: 0.0,
        }
    }

    pub fn ring_buffer_push(&mut self, item: GpsReaderResults) {
        if !self.stack.is_full() {
            let _ = self.stack.push_back(item);
        } else {
            Deque::pop_front(&mut self.stack);
            let _ = self.stack.push_back(item);
        }
    }

    pub fn add_coords(&mut self, coords: GpsReaderResults, mut _last_lla: Option<GpsReaderResults>, is_recording: bool) {
        if let GpsReaderResults {
            lat: Some(new_lat),
            lon: Some(new_lon),
            alt: Some(new_alt),
            hdop: Some(new_hdop),
            timestamp: Some(new_timestamp)
        } = coords {
            self.current_hdop = new_hdop;
            if let Some(last_coord) = self.stack.back() {
                if let GpsReaderResults {
                    lat: Some(prev_lat),
                    lon: Some(prev_lon),
                    alt: Some(prev_alt),
                    hdop: Some(_prev_hdop),
                    timestamp: Some(prev_timestamp)
                } = *last_coord {
                    let time_delta = new_timestamp - prev_timestamp;
                    if time_delta < Duration::milliseconds(self.min_time_interval_ms) {
                        return;
                    }

                    if new_hdop < 5.0 {
                        _last_lla = Some(coords);
                        let p1 = LatLonAlt {
                            latitude: prev_lat,
                            longitude: prev_lon,
                            altitude: prev_alt,
                        };
                        let p2 = LatLonAlt {
                            latitude: new_lat,
                            longitude: new_lon,
                            altitude: new_alt,
                        };

                        let distance_segment_ft = haversine_distance_ft(p1, p2);

                        let alt_diff = p2.altitude - p1.altitude;

                        self.ring_buffer_push(coords);

                         if is_recording {
                            self.last_segment_distance = distance_segment_ft;
                            self.total_distance += distance_segment_ft;
                            if alt_diff > 0.0 {
                                self.total_elevation_gain += alt_diff
                            }
                        }

                        if distance_segment_ft > self.min_distance_threshold {
                            self.current_speed_mph =
                                calculate_speed(distance_segment_ft, time_delta.as_seconds_f64());
                        } else {
                            self.current_speed_mph = 0.0;
                        }
                    }
                }
            } else {
                self.ring_buffer_push(coords);
            }
        }
    }
}
