//! Google's encoded-polyline algorithm (tracking doc §4): the Directions
//! response stores the route as an encoded string that clients decode for
//! drawing, and the server decodes for the stray check. Hand-written — the
//! algorithm is fully specified and the round-trip test travels with it.

use crate::Coord;

/// Decode a Google encoded polyline into its points, in route order. A
/// truncated string yields only its complete prefix — malformed geometry
/// never crashes anything downstream.
pub fn decode_polyline(encoded: &str) -> Vec<Coord> {
    let bytes = encoded.as_bytes();
    let mut points = Vec::new();
    let mut index = 0usize;
    let (mut lat, mut lng): (i64, i64) = (0, 0);

    while index < bytes.len() {
        let Some(lat_delta) = read_component(bytes, &mut index) else {
            break;
        };
        let Some(lng_delta) = read_component(bytes, &mut index) else {
            break;
        };
        lat += lat_delta;
        lng += lng_delta;
        points.push(Coord {
            lat: lat as f64 / 1e5,
            lng: lng as f64 / 1e5,
        });
    }
    points
}

/// One coordinate component: 5-bit little-endian chunks (+63, the last
/// without the continuation bit), sign folded into the LSB, delta-accumulated.
fn read_component(bytes: &[u8], index: &mut usize) -> Option<i64> {
    let mut result: u64 = 0;
    let mut shift = 0u32;
    loop {
        if *index >= bytes.len() {
            return None;
        }
        let byte = (bytes[*index].wrapping_sub(63)) as u64;
        *index += 1;
        result |= (byte & 0x1f) << shift;
        shift += 5;
        if byte & 0x20 == 0 {
            break;
        }
    }
    // The LSB folds the sign; undo the fold, then shift back.
    let delta = if result & 1 == 1 {
        !((result >> 1) as i64)
    } else {
        (result >> 1) as i64
    };
    Some(delta)
}

/// Encode points back into Google's polyline format — the inverse, used
/// by tests to build fixtures and by nothing else today.
pub fn encode_polyline(points: &[Coord]) -> String {
    let mut out = String::new();
    let (mut prev_lat, mut prev_lng): (i64, i64) = (0, 0);
    for point in points {
        let lat = (point.lat * 1e5).round() as i64;
        let lng = (point.lng * 1e5).round() as i64;
        write_component(&mut out, lat - prev_lat);
        write_component(&mut out, lng - prev_lng);
        prev_lat = lat;
        prev_lng = lng;
    }
    out
}

fn write_component(out: &mut String, delta: i64) {
    // Left-shift folds the sign into the LSB: negative deltas invert the
    // shifted value (Google's spec), making the odd/even LSB the sign bit.
    let shifted: i64 = delta.wrapping_shl(1);
    let mut remaining: u64 = if delta < 0 {
        !(shifted as u64)
    } else {
        shifted as u64
    };
    loop {
        let mut chunk = remaining & 0x1f;
        remaining >>= 5;
        if remaining > 0 {
            chunk |= 0x20;
        }
        out.push((chunk + 63) as u8 as char);
        if remaining == 0 {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Google's published example points (their docs' 3-point list):
    /// encode → decode must agree, negatives and all. The truncated
    /// variant of the string decodes only its complete points — the
    /// honest-truncation test below covers that separately.
    #[test]
    fn round_trips_the_canonical_google_points() {
        let points = vec![
            Coord {
                lat: 38.5,
                lng: -120.2,
            },
            Coord {
                lat: 40.7,
                lng: -120.95,
            },
            Coord {
                lat: 43.252,
                lng: -126.453,
            },
        ];
        let decoded = decode_polyline(&encode_polyline(&points));
        assert_eq!(decoded.len(), points.len());
        for (decoded, original) in decoded.iter().zip(&points) {
            assert!((decoded.lat - original.lat).abs() < 1e-5);
            assert!((decoded.lng - original.lng).abs() < 1e-5);
        }
    }

    /// Round-trip over the Kigali fixtures — negatives, sub-1e5 jitter,
    /// and the cross-hemisphere signs the bit-folding must survive.
    #[test]
    fn round_trips_kigali_and_jitter() {
        let points = vec![
            Coord {
                lat: -1.9620,
                lng: 30.1290,
            },
            Coord {
                lat: -1.96201,
                lng: 30.12901,
            },
            Coord {
                lat: -1.9499,
                lng: 30.0622,
            },
            Coord {
                lat: 43.252,
                lng: -126.453,
            },
            Coord {
                lat: -0.00001,
                lng: 0.00001,
            },
        ];
        let decoded = decode_polyline(&encode_polyline(&points));
        assert_eq!(decoded.len(), points.len());
        for (decoded, original) in decoded.iter().zip(&points) {
            assert!((decoded.lat - original.lat).abs() < 1e-5);
            assert!((decoded.lng - original.lng).abs() < 1e-5);
        }
    }

    #[test]
    fn empty_and_truncated_inputs_are_honest() {
        assert!(decode_polyline("").is_empty());
        // A single truncated component decodes to nothing, not a crash.
        assert!(decode_polyline("?").is_empty());
    }
}
