//! Proyeksi UTM pada datum WGS84 (rumus seri Snyder / Krüger orde-6).

use crate::geo::Coord;

const SEMI_MAJOR: f64 = 6_378_137.0;
const FLATTENING: f64 = 1.0 / 298.257_223_563;
const K0: f64 = 0.9996;
const E2: f64 = FLATTENING * (2.0 - FLATTENING);
const EP2: f64 = E2 / (1.0 - E2);

pub const FALSE_EASTING: f64 = 500_000.0;
pub const FALSE_NORTHING_SOUTH: f64 = 10_000_000.0;
/// Rentang zona UTM yang dipakai untuk input pengguna di Indonesia.
pub const INDONESIA_ZONE_MIN: u8 = 46;
pub const INDONESIA_ZONE_MAX: u8 = 54;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hemisphere {
    North,
    South,
}

impl Hemisphere {
    pub fn letter(self) -> char {
        match self {
            Hemisphere::North => 'N',
            Hemisphere::South => 'S',
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Utm {
    pub zone: u8,
    pub hemisphere: Hemisphere,
    pub easting: f64,
    pub northing: f64,
}

/// Nomor zona UTM (1–60) untuk bujur tertentu.
pub fn zone_for_lon(lon: f64) -> u8 {
    (((lon + 180.0) / 6.0).floor() as i64 + 1).clamp(1, 60) as u8
}

/// Meridian tengah zona UTM dalam derajat.
pub fn central_meridian(zone: u8) -> f64 {
    zone as f64 * 6.0 - 183.0
}

fn meridian_arc(phi: f64) -> f64 {
    let e4 = E2 * E2;
    let e6 = e4 * E2;
    SEMI_MAJOR
        * ((1.0 - E2 / 4.0 - 3.0 * e4 / 64.0 - 5.0 * e6 / 256.0) * phi
            - (3.0 * E2 / 8.0 + 3.0 * e4 / 32.0 + 45.0 * e6 / 1024.0) * (2.0 * phi).sin()
            + (15.0 * e4 / 256.0 + 45.0 * e6 / 1024.0) * (4.0 * phi).sin()
            - (35.0 * e6 / 3072.0) * (6.0 * phi).sin())
}

/// Proyeksi mentah: easting relatif terhadap meridian tengah (tanpa 500 km)
/// dan northing bertanda (tanpa false northing). Aman untuk menghitung luas
/// polygon yang melintasi khatulistiwa.
pub fn project_raw(lon: f64, lat: f64, lon0: f64) -> (f64, f64) {
    let phi = lat.to_radians();
    let lam = (lon - lon0).to_radians();
    let (sin, cos, tan) = (phi.sin(), phi.cos(), phi.tan());
    let n = SEMI_MAJOR / (1.0 - E2 * sin * sin).sqrt();
    let t = tan * tan;
    let c = EP2 * cos * cos;
    let a = lam * cos;
    let m = meridian_arc(phi);
    let easting = K0
        * n
        * (a
            + (1.0 - t + c) * a.powi(3) / 6.0
            + (5.0 - 18.0 * t + t * t + 72.0 * c - 58.0 * EP2) * a.powi(5) / 120.0);
    let northing = K0
        * (m
            + n * tan
                * (a * a / 2.0
                    + (5.0 - t + 9.0 * c + 4.0 * c * c) * a.powi(4) / 24.0
                    + (61.0 - 58.0 * t + t * t + 600.0 * c - 330.0 * EP2) * a.powi(6) / 720.0));
    (easting, northing)
}

/// Konversi WGS84 (derajat) ke UTM dengan zona otomatis dan false easting/northing.
pub fn forward(lon: f64, lat: f64) -> Utm {
    let zone = zone_for_lon(lon);
    let (e, n) = project_raw(lon, lat, central_meridian(zone));
    let hemisphere = if lat < 0.0 {
        Hemisphere::South
    } else {
        Hemisphere::North
    };
    let northing = match hemisphere {
        Hemisphere::North => n,
        Hemisphere::South => n + FALSE_NORTHING_SOUTH,
    };
    Utm {
        zone,
        hemisphere,
        easting: e + FALSE_EASTING,
        northing,
    }
}

/// Konversi UTM kembali ke `[lon, lat]` WGS84.
pub fn inverse(zone: u8, hemisphere: Hemisphere, easting: f64, northing: f64) -> Coord {
    let x = easting - FALSE_EASTING;
    let y = match hemisphere {
        Hemisphere::North => northing,
        Hemisphere::South => northing - FALSE_NORTHING_SOUTH,
    };
    let e1 = (1.0 - (1.0 - E2).sqrt()) / (1.0 + (1.0 - E2).sqrt());
    let m = y / K0;
    let mu = m / (SEMI_MAJOR * (1.0 - E2 / 4.0 - 3.0 * E2 * E2 / 64.0 - 5.0 * E2.powi(3) / 256.0));
    let phi1 = mu
        + (3.0 * e1 / 2.0 - 27.0 * e1.powi(3) / 32.0) * (2.0 * mu).sin()
        + (21.0 * e1 * e1 / 16.0 - 55.0 * e1.powi(4) / 32.0) * (4.0 * mu).sin()
        + (151.0 * e1.powi(3) / 96.0) * (6.0 * mu).sin()
        + (1097.0 * e1.powi(4) / 512.0) * (8.0 * mu).sin();
    let (s, c, tan) = (phi1.sin(), phi1.cos(), phi1.tan());
    let n1 = SEMI_MAJOR / (1.0 - E2 * s * s).sqrt();
    let t1 = tan * tan;
    let c1 = EP2 * c * c;
    let r1 = SEMI_MAJOR * (1.0 - E2) / (1.0 - E2 * s * s).powf(1.5);
    let d = x / (n1 * K0);
    let phi = phi1
        - (n1 * tan / r1)
            * (d * d / 2.0
                - (5.0 + 3.0 * t1 + 10.0 * c1 - 4.0 * c1 * c1 - 9.0 * EP2) * d.powi(4) / 24.0
                + (61.0 + 90.0 * t1 + 298.0 * c1 + 45.0 * t1 * t1 - 252.0 * EP2 - 3.0 * c1 * c1)
                    * d.powi(6)
                    / 720.0);
    let lam = (d
        - (1.0 + 2.0 * t1 + c1) * d.powi(3) / 6.0
        + (5.0 - 2.0 * c1 + 28.0 * t1 - 3.0 * c1 * c1 + 8.0 * EP2 + 24.0 * t1 * t1) * d.powi(5)
            / 120.0)
        / c;
    [central_meridian(zone) + lam.to_degrees(), phi.to_degrees()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jakarta_lands_in_zone_48_south_with_plausible_coordinates() {
        let u = forward(106.8272, -6.1754);
        assert_eq!(u.zone, 48);
        assert_eq!(u.hemisphere, Hemisphere::South);
        assert!((700_000.0..706_000.0).contains(&u.easting), "E={}", u.easting);
        assert!((9_314_000.0..9_320_000.0).contains(&u.northing), "N={}", u.northing);
    }

    #[test]
    fn round_trip_is_precise() {
        for &(lon, lat) in &[(106.8272, -6.1754), (117.5, 0.5), (96.1, 5.5), (140.7, -2.5)] {
            let u = forward(lon, lat);
            let [lon2, lat2] = inverse(u.zone, u.hemisphere, u.easting, u.northing);
            assert!((lon - lon2).abs() < 1e-7, "lon {lon} vs {lon2}");
            assert!((lat - lat2).abs() < 1e-7, "lat {lat} vs {lat2}");
        }
    }

    #[test]
    fn zone_boundaries() {
        assert_eq!(zone_for_lon(-180.0), 1);
        assert_eq!(zone_for_lon(180.0), 60);
        assert_eq!(zone_for_lon(106.8), 48);
    }
}
