//! Parsing dan format koordinat: Desimal Derajat (DD), DMS, UTM (zona 46–54, WGS84).

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::geo::Coord;
use crate::utm::{self, Hemisphere, INDONESIA_ZONE_MAX, INDONESIA_ZONE_MIN};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Lat,
    Lon,
}

#[derive(Debug, Error, PartialEq)]
pub enum CoordError {
    #[error("format koordinat tidak dikenali: {0}")]
    Format(String),
    #[error("nilai di luar rentang: {0}")]
    Range(String),
    #[error("zona UTM harus 46–54 untuk Indonesia, diterima {0}")]
    Zone(u8),
}

fn axis_name(axis: Axis) -> &'static str {
    match axis {
        Axis::Lat => "lintang",
        Axis::Lon => "bujur",
    }
}

fn parse_number(text: &str) -> Result<f64, CoordError> {
    let normalized = text.trim().replace(',', ".");
    match normalized.parse::<f64>() {
        Ok(v) if v.is_finite() => Ok(v),
        _ => Err(CoordError::Format(text.trim().to_string())),
    }
}

fn check_range(value: f64, max_abs: f64, axis: Axis) -> Result<f64, CoordError> {
    if value.abs() > max_abs {
        Err(CoordError::Range(format!("{} {value}", axis_name(axis))))
    } else {
        Ok(value)
    }
}

/// Desimal derajat dari dua isian terpisah (lintang, bujur). Hasil `[lon, lat]`.
pub fn parse_dd(lat: &str, lon: &str) -> Result<Coord, CoordError> {
    let lat = check_range(parse_number(lat)?, 90.0, Axis::Lat)?;
    let lon = check_range(parse_number(lon)?, 180.0, Axis::Lon)?;
    Ok([lon, lat])
}

/// Desimal derajat dari satu teks `lintang, bujur` (pemisah koma, titik koma, atau spasi).
pub fn parse_dd_pair(text: &str) -> Result<Coord, CoordError> {
    // Satu koma = pemisah; lebih dari satu koma = koma desimal, dipisah spasi.
    let parts: Vec<&str> = if text.contains(';') {
        text.split(';').collect()
    } else if text.matches(',').count() == 1 {
        text.split(',').collect()
    } else {
        text.split_whitespace().collect()
    };
    match parts.as_slice() {
        [lat, lon] => parse_dd(lat.trim(), lon.trim()),
        _ => Err(CoordError::Format(format!(
            "{text} (gunakan format seperti -6.2, 106.8)"
        ))),
    }
}

/// Satu komponen DMS, contoh: `6°12'30.5"S`, `6 12 30.5 S`, atau `-6 12`.
/// Hasil desimal derajat bertanda (S dan W menjadi negatif).
pub fn parse_dms(text: &str, axis: Axis) -> Result<f64, CoordError> {
    let mut tokens: Vec<String> = Vec::new();
    let mut buf = String::new();
    for ch in text.chars() {
        if ch.is_ascii_digit() || matches!(ch, '.' | ',' | '-' | '+') {
            buf.push(ch);
            continue;
        }
        if !buf.is_empty() {
            tokens.push(std::mem::take(&mut buf));
        }
        if ch.is_ascii_alphabetic() {
            tokens.push(ch.to_ascii_uppercase().to_string());
        } else if !(ch.is_whitespace() || "°º'\"′″’‘”“".contains(ch)) {
            return Err(CoordError::Format(text.to_string()));
        }
    }
    if !buf.is_empty() {
        tokens.push(buf);
    }

    let mut numbers = Vec::new();
    let mut letters = Vec::new();
    for token in &tokens {
        if token.chars().all(|c| c.is_ascii_alphabetic()) {
            letters.push(token.chars().next().unwrap_or(' '));
        } else {
            numbers.push(parse_number(token)?);
        }
    }
    if letters.len() > 1 || numbers.is_empty() || numbers.len() > 3 {
        return Err(CoordError::Format(text.to_string()));
    }
    let hemisphere = letters.first().copied();
    let (max_deg, valid_hemi) = match axis {
        Axis::Lat => (90.0, ['N', 'S']),
        Axis::Lon => (180.0, ['E', 'W']),
    };
    if let Some(h) = hemisphere {
        if !valid_hemi.contains(&h) {
            return Err(CoordError::Format(format!(
                "arah {h} tidak sesuai untuk {}",
                axis_name(axis)
            )));
        }
    }

    let deg = numbers[0];
    let min = numbers.get(1).copied().unwrap_or(0.0);
    let sec = numbers.get(2).copied().unwrap_or(0.0);
    if !(0.0..60.0).contains(&min) || !(0.0..60.0).contains(&sec) {
        return Err(CoordError::Range(format!(
            "menit dan detik harus 0 sampai kurang dari 60 ({text})"
        )));
    }
    let negative_hemi = matches!(hemisphere, Some('S') | Some('W'));
    if negative_hemi && deg.is_sign_negative() {
        return Err(CoordError::Format(format!(
            "tanda minus dan arah S/W dipakai bersamaan ({text})"
        )));
    }
    let magnitude = deg.abs() + min / 60.0 + sec / 3600.0;
    if magnitude > max_deg {
        return Err(CoordError::Range(format!(
            "{} {magnitude}",
            axis_name(axis)
        )));
    }
    let negative = deg.is_sign_negative() || negative_hemi;
    Ok(if negative { -magnitude } else { magnitude })
}

/// UTM dari teks seperti `48S 703000 9317000` (atau `48 S 703000 9317000`). Hasil `[lon, lat]`.
pub fn parse_utm(text: &str) -> Result<Coord, CoordError> {
    let spaced: String = text
        .trim()
        .to_ascii_uppercase()
        .chars()
        .map(|c| if c == ',' { ' ' } else { c })
        .collect();
    let mut parts: Vec<String> = spaced.split_whitespace().map(str::to_string).collect();
    if let Some(first) = parts.first().cloned() {
        if first.len() > 1 && first.ends_with(|c: char| c == 'N' || c == 'S') {
            let (zone_part, hemi_part) = first.split_at(first.len() - 1);
            parts[0] = zone_part.to_string();
            parts.insert(1, hemi_part.to_string());
        }
    }
    let [zone_s, hemi_s, e_s, n_s] = parts.as_slice() else {
        return Err(CoordError::Format(format!(
            "{text} (gunakan format seperti 48S 703000 9317000)"
        )));
    };
    let zone: u8 = zone_s
        .parse()
        .map_err(|_| CoordError::Format(text.to_string()))?;
    if !(INDONESIA_ZONE_MIN..=INDONESIA_ZONE_MAX).contains(&zone) {
        return Err(CoordError::Zone(zone));
    }
    let hemisphere = match hemi_s.as_str() {
        "N" => Hemisphere::North,
        "S" => Hemisphere::South,
        _ => return Err(CoordError::Format(text.to_string())),
    };
    let easting = parse_number(e_s)?;
    let northing = parse_number(n_s)?;
    if !(100_000.0..=900_000.0).contains(&easting) {
        return Err(CoordError::Range(format!("easting {easting}")));
    }
    if !(0.0..=10_000_000.0).contains(&northing) {
        return Err(CoordError::Range(format!("northing {northing}")));
    }
    Ok(utm::inverse(zone, hemisphere, easting, northing))
}

/// Format DMS dengan dua desimal detik, contoh `6°12'30.00"S`.
pub fn format_dms(value: f64, axis: Axis) -> String {
    let hemi = match (axis, value < 0.0) {
        (Axis::Lat, false) => 'N',
        (Axis::Lat, true) => 'S',
        (Axis::Lon, false) => 'E',
        (Axis::Lon, true) => 'W',
    };
    // satuan 0,01 detik agar pembulatan tidak menghasilkan "60"
    let total = (value.abs() * 360_000.0).round() as u64;
    let deg = total / 360_000;
    let rem = total % 360_000;
    let min = rem / 6_000;
    let sec = (rem % 6_000) as f64 / 100.0;
    format!("{deg}°{min:02}'{sec:05.2}\"{hemi}")
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "mode", rename_all = "lowercase")]
pub enum ConvertInput {
    Dd { lat: String, lon: String },
    Dms { lat: String, lon: String },
    Utm { text: String },
    Pair { text: String },
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct DmsOutput {
    pub lat: String,
    pub lon: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct UtmOutput {
    pub zone: u8,
    pub hemisphere: char,
    pub easting: f64,
    pub northing: f64,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ConvertOutput {
    pub lat: f64,
    pub lon: f64,
    pub dd: String,
    pub dms: DmsOutput,
    pub utm: UtmOutput,
}

/// Konversi satu input ke ketiga format sekaligus.
pub fn convert(input: &ConvertInput) -> Result<ConvertOutput, CoordError> {
    let [lon, lat] = match input {
        ConvertInput::Dd { lat, lon } => parse_dd(lat, lon)?,
        ConvertInput::Dms { lat, lon } => {
            let lat = parse_dms(lat, Axis::Lat)?;
            let lon = parse_dms(lon, Axis::Lon)?;
            [lon, lat]
        }
        ConvertInput::Utm { text } => parse_utm(text)?,
        ConvertInput::Pair { text } => parse_dd_pair(text)?,
    };
    Ok(from_dd(lon, lat))
}

/// Susun seluruh representasi dari koordinat desimal `[lon, lat]`.
pub fn from_dd(lon: f64, lat: f64) -> ConvertOutput {
    let u = utm::forward(lon, lat);
    let (e, n) = (round2(u.easting), round2(u.northing));
    ConvertOutput {
        lat,
        lon,
        dd: format!("{lat:.6}, {lon:.6}"),
        dms: DmsOutput {
            lat: format_dms(lat, Axis::Lat),
            lon: format_dms(lon, Axis::Lon),
        },
        utm: UtmOutput {
            zone: u.zone,
            hemisphere: u.hemisphere.letter(),
            easting: e,
            northing: n,
            text: format!("{}{} {e:.2} {n:.2}", u.zone, u.hemisphere.letter()),
        },
    }
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dms_component_parses_with_hemisphere() {
        let lat = parse_dms("6°12'30\"S", Axis::Lat).unwrap();
        assert!((lat - (-(6.0 + 12.0 / 60.0 + 30.0 / 3600.0))).abs() < 1e-12);
        let lon = parse_dms("106 48 22 E", Axis::Lon).unwrap();
        assert!((lon - (106.0 + 48.0 / 60.0 + 22.0 / 3600.0)).abs() < 1e-12);
    }

    #[test]
    fn dms_rejects_wrong_hemisphere_and_bad_minutes() {
        assert!(matches!(parse_dms("6°12'30\"E", Axis::Lat), Err(CoordError::Format(_))));
        assert!(matches!(parse_dms("6°75'00\"S", Axis::Lat), Err(CoordError::Range(_))));
        assert!(matches!(parse_dms("-6°12'S", Axis::Lat), Err(CoordError::Format(_))));
    }

    #[test]
    fn dd_pair_and_range() {
        assert_eq!(parse_dd_pair("-6.2, 106.8").unwrap(), [106.8, -6.2]);
        assert_eq!(parse_dd_pair("-6,2 106,8").unwrap(), [106.8, -6.2]);
        assert!(matches!(parse_dd("95", "106"), Err(CoordError::Range(_))));
    }

    #[test]
    fn utm_text_parses_and_rejects_zone_outside_indonesia() {
        let [lon, lat] = parse_utm("48S 703000 9317000").unwrap();
        assert!((106.7..106.9).contains(&lon), "lon {lon}");
        assert!((-6.2..-6.1).contains(&lat), "lat {lat}");
        let [lon2, lat2] = parse_utm("48 S, 703000, 9317000").unwrap();
        assert!((lon - lon2).abs() < 1e-12 && (lat - lat2).abs() < 1e-12);
        assert_eq!(parse_utm("40S 703000 9317000"), Err(CoordError::Zone(40)));
    }

    #[test]
    fn format_dms_is_stable() {
        assert_eq!(format_dms(-6.208_333_333, Axis::Lat), "6°12'30.00\"S");
        assert_eq!(format_dms(106.8, Axis::Lon), "106°48'00.00\"E");
    }

    #[test]
    fn convert_fills_all_formats() {
        let out = convert(&ConvertInput::Pair {
            text: "-6.1754, 106.8272".into(),
        })
        .unwrap();
        assert_eq!(out.utm.zone, 48);
        assert_eq!(out.utm.hemisphere, 'S');
        assert!(out.utm.text.starts_with("48S "));
        assert!(out.dms.lon.ends_with('E'));
    }
}
