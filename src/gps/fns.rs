use libm::{atan2, cos, sin, sqrt};
use core::fmt::Error;

#[derive(Debug, Copy, Clone)]
pub struct LatLonAlt {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: f32,
}

const EARTH_RADIUS_M: f64 = 6371000.0;
const FT_PER_METER: f64 = 3.28084;
const FT_IN_A_MILE: f64 = 5280.0;

fn to_radians(degrees: f64) -> f64 {
    degrees * (core::f64::consts::PI / 180.0)
}

pub fn haversine_distance_ft(p1: LatLonAlt, p2: LatLonAlt) -> f64 {
    let lat1_rad = to_radians(p1.latitude);
    let lon1_rad = to_radians(p1.longitude);
    let lat2_rad = to_radians(p2.latitude);
    let lon2_rad = to_radians(p2.longitude);

    let d_lat = lat2_rad - lat1_rad;
    let d_lon = lon2_rad - lon1_rad;

    let sin_dlat = sin(d_lat / 2.0);
    let sin_dlon = sin(d_lon / 2.0);

    let a = sin_dlat * sin_dlat + cos(lat1_rad) * cos(lat2_rad) * sin_dlon * sin_dlon;

    let c = 2.0 * atan2(sqrt(a), sqrt(1.0 - a));

    EARTH_RADIUS_M * FT_PER_METER * c
}

pub fn calculate_speed(distance_ft: f64, time_secs: f64) -> f64 {
    let fps_to_mph_conversion_factor = 3600.0 / FT_IN_A_MILE;
    if time_secs == 0.0 {
        return 0.0;
    }
    let speed_fps = distance_ft / time_secs;
    speed_fps * fps_to_mph_conversion_factor
}

pub fn to_feet(meters: f64) -> f64 {
    return FT_PER_METER * meters;
}