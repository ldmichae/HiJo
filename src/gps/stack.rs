use heapless::Vec;

use crate::gps::{
    fns::{LatLon, calculate_speed, haversine_distance_ft},
    reader::GpsReaderResults,
};

const MAX_ITEMS: usize = 16;

pub struct GeoStack {
    pub stack: Vec<GpsReaderResults, MAX_ITEMS>,
    pub last_segment_distance: f64,
    pub total_distance: f64,
    pub current_speed_mph: f64,
    pub current_hdop: f32,
}

impl GeoStack {
    pub fn new() -> Self {
        GeoStack {
            stack: Vec::new(),
            last_segment_distance: 0.0,
            total_distance: 0.0,
            current_speed_mph: 0.0,
            current_hdop: 0.0,
        }
    }

    pub fn add_coords(&mut self, coords: GpsReaderResults) {
        // Make sure the new coordinates have valid lat/lon
        if let (Some(new_lat), Some(new_lon), Some(hdop)) = (coords.lat, coords.lon, coords.hdop) {
            if let Some(last_coord) = self.stack.last() {
                if let (Some(prev_lat), Some(prev_lon)) = (last_coord.lat, last_coord.lon) {
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

                    if hdop <= 2.0 || distance_segment_ft > 6.0 {
                        self.last_segment_distance = distance_segment_ft;
                        self.total_distance += distance_segment_ft;
                        self.current_speed_mph = calculate_speed(distance_segment_ft);
                    }
                }
            }

            // Only push if coordinates are valid
            let _ = self.stack.push(coords);
        }

        if let Some(hdop) = coords.hdop {
            self.current_hdop = hdop
        }
    }
}
