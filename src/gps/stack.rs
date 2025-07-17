use chrono::Duration;
use heapless::{Deque, Vec};

use crate::gps::{
    fns::{LatLon, calculate_speed, haversine_distance_ft},
    reader::GpsReaderResults,
};

const MAX_ITEMS: usize = 16;

pub struct GeoStack {
    pub stack: Deque<GpsReaderResults, MAX_ITEMS>,
    pub last_segment_distance: f64,
    pub total_distance: f64,
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
            current_speed_mph: 0.0,
            current_hdop: 0.0,
            min_time_interval_ms: 1000,
            min_distance_threshold: 1.0,
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

    pub fn add_coords(&mut self, coords: GpsReaderResults) {
        // Make sure the new coordinates have valid lat/lon
        if let (Some(new_lat), Some(new_lon), Some(hdop), Some(new_timestamp)) =
            (coords.lat, coords.lon, coords.hdop, coords.timestamp)
        {
            self.current_hdop = hdop;
            if let Some(last_coord) = self.stack.back() {
                if let (Some(prev_lat), Some(prev_lon), Some(prev_timestamp)) =
                    (last_coord.lat, last_coord.lon, last_coord.timestamp)
                {
                    let time_delta = new_timestamp - prev_timestamp;
                    if time_delta < Duration::milliseconds(self.min_time_interval_ms) {
                        return; // Skip this reading
                    }

                    if hdop < 2.0 {
                        let distance_segment_ft = haversine_distance_ft(
                            LatLon {
                                latitude: prev_lat,
                                longitude: prev_lon,
                            },
                            LatLon {
                                latitude: new_lat,
                                longitude: new_lon,
                            },
                        );

                        if distance_segment_ft > self.min_distance_threshold {
                            let _ = self.ring_buffer_push(coords);
                            self.last_segment_distance = distance_segment_ft;
                            self.total_distance += distance_segment_ft;
                            self.current_speed_mph =
                                calculate_speed(distance_segment_ft, time_delta.as_seconds_f64());
                        }
                    }
                }
            }
            let _ = self.ring_buffer_push(coords);
        }
    }
}
