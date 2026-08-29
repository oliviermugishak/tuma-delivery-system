//! Geo: pure distance/ETA math for the customer's "Stores near you".
//!
//! Server-owned truth: the client sends its location, the server computes
//! distance (haversine — fine at Kigali scale; PostGIS only when zone or
//! radius queries become real) and the ride-time ETA. Kigali's effective
//! moto speed (~25 km/h door to door) is the one business constant here;
//! if traffic data ever earns its place, it lands in this file.

/// Earth radius in meters (mean).
const EARTH_RADIUS_M: f64 = 6_371_000.0;

/// The one business constant: effective motorcycle speed across Kigali,
/// km/h door to door. ETA is pure ride time — preparation varies per store
/// and never bleeds into this number.
const MOTO_SPEED_KMH: f64 = 25.0;

/// Great-circle distance between two WGS84 points, in whole meters.
pub fn haversine_m(lat1: f64, lng1: f64, lat2: f64, lng2: f64) -> i64 {
    let (lat1, lng1, lat2, lng2) = (
        lat1.to_radians(),
        lng1.to_radians(),
        lat2.to_radians(),
        lng2.to_radians(),
    );
    let d_lat = lat2 - lat1;
    let d_lng = lng2 - lng1;
    let a = (d_lat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (d_lng / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
    (EARTH_RADIUS_M * c).round() as i64
}

/// Pure ride time for a distance, in whole minutes (minimum 1 — a store
/// across the street still takes a minute).
pub fn ride_minutes(distance_m: i64) -> i64 {
    let km = distance_m as f64 / 1000.0;
    (km / MOTO_SPEED_KMH * 60.0).ceil().max(1.0) as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real Kigali pair: Kigali Heights (Kimihurura) → Union Trade Center
    /// (city center) — roughly 3.4 km by straight line.
    const KIGALI_HEIGHTS: (f64, f64) = (-1.9410, 30.0912);
    const UTC_CITY_CENTER: (f64, f64) = (-1.9499, 30.0622);

    #[test]
    fn haversine_matches_known_kigali_distance() {
        let meters = haversine_m(
            KIGALI_HEIGHTS.0,
            KIGALI_HEIGHTS.1,
            UTC_CITY_CENTER.0,
            UTC_CITY_CENTER.1,
        );
        assert!(
            (3_300..=3_450).contains(&meters),
            "expected ~3.4 km, got {meters} m"
        );
    }

    #[test]
    fn haversine_is_symmetric_and_zero_for_same_point() {
        let ab = haversine_m(
            KIGALI_HEIGHTS.0,
            KIGALI_HEIGHTS.1,
            UTC_CITY_CENTER.0,
            UTC_CITY_CENTER.1,
        );
        let ba = haversine_m(
            UTC_CITY_CENTER.0,
            UTC_CITY_CENTER.1,
            KIGALI_HEIGHTS.0,
            KIGALI_HEIGHTS.1,
        );
        assert_eq!(ab, ba);
        assert_eq!(haversine_m(-1.95, 30.06, -1.95, 30.06), 0);
    }

    #[test]
    fn ride_minutes_ceil_and_floor() {
        assert_eq!(ride_minutes(0), 1, "across the street still takes a minute");
        // 3,370 m at 25 km/h = 8.088 min → ceil 9.
        assert_eq!(ride_minutes(3_370), 9);
        // Exactly 25 km = 60 min.
        assert_eq!(ride_minutes(25_000), 60);
    }
}
