//! Penulisan dan pembacaan berkas SHP (tipe Polygon) serta SHX.
//! Mengikuti ESRI Shapefile Technical Description. Hanya satu ring luar per fitur.

use thiserror::Error;

use crate::geo::{self, Coord};

pub const SHAPE_POLYGON: i32 = 5;
const FILE_CODE: i32 = 9994;
const VERSION: i32 = 1000;
const HEADER_LEN: usize = 100;
const RECORD_HEADER_LEN: usize = 8;

#[derive(Debug, Error, PartialEq)]
pub enum ShpError {
    #[error("ring poligon harus tertutup dan memiliki minimal 4 titik")]
    RingNotClosed,
    #[error("ring luar harus searah jarum jam (standar ESRI)")]
    NotClockwise,
    #[error("berkas SHP tidak valid: {0}")]
    Invalid(String),
    #[error("tipe bentuk {0} belum didukung (hanya Polygon = 5)")]
    UnsupportedType(i32),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShpOutput {
    pub shp: Vec<u8>,
    pub shx: Vec<u8>,
    /// `[minLon, minLat, maxLon, maxLat]`
    pub bbox: [f64; 4],
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShpData {
    pub shape_type: i32,
    pub bbox: [f64; 4],
    /// Fitur -> bagian (ring) -> titik. Ring luar pertama pada indeks 0.
    pub features: Vec<Vec<Vec<Coord>>>,
}

/// Tulis SHP + SHX dari daftar ring luar. Setiap ring harus tertutup dan searah jarum jam.
pub fn write_polygons(rings: &[Vec<Coord>]) -> Result<ShpOutput, ShpError> {
    let mut contents = Vec::with_capacity(rings.len());
    for ring in rings {
        if ring.len() < 4 || ring.first() != ring.last() {
            return Err(ShpError::RingNotClosed);
        }
        if geo::signed_area_planar(ring) > 0.0 {
            return Err(ShpError::NotClockwise);
        }
        contents.push(polygon_content(ring));
    }

    let bbox = union_bbox(rings);
    let shp_len = HEADER_LEN
        + contents
            .iter()
            .map(|c| RECORD_HEADER_LEN + c.len())
            .sum::<usize>();
    let shx_len = HEADER_LEN + RECORD_HEADER_LEN * contents.len();

    let mut shp = header(shp_len, bbox);
    let mut shx = header(shx_len, bbox);
    let mut offset = HEADER_LEN;
    for (i, content) in contents.iter().enumerate() {
        let content_words = (content.len() / 2) as i32;
        shp.extend_from_slice(&((i + 1) as i32).to_be_bytes());
        shp.extend_from_slice(&content_words.to_be_bytes());
        shp.extend_from_slice(content);

        shx.extend_from_slice(&((offset / 2) as i32).to_be_bytes());
        shx.extend_from_slice(&content_words.to_be_bytes());
        offset += RECORD_HEADER_LEN + content.len();
    }
    Ok(ShpOutput { shp, shx, bbox })
}

/// Baca SHP (tipe Polygon atau Null) menjadi fitur-fitur ring.
pub fn read_polygons(bytes: &[u8]) -> Result<ShpData, ShpError> {
    let header = parse_header(bytes)?;
    if header.shape_type != SHAPE_POLYGON && header.shape_type != 0 {
        return Err(ShpError::UnsupportedType(header.shape_type));
    }

    let mut features = Vec::new();
    let mut pos = HEADER_LEN;
    while pos < bytes.len() {
        let record_no = be_i32(bytes, pos).ok_or_else(|| invalid("header record terpotong"))?;
        if record_no as usize != features.len() + 1 {
            return Err(invalid("nomor record tidak berurutan"));
        }
        let words = be_i32(bytes, pos + 4).ok_or_else(|| invalid("header record terpotong"))?;
        if words < 0 {
            return Err(invalid("panjang record negatif"));
        }
        let start = pos + RECORD_HEADER_LEN;
        let end = start
            .checked_add((words as usize).checked_mul(2).ok_or_else(|| invalid("panjang record tidak valid"))?)
            .ok_or_else(|| invalid("panjang record tidak valid"))?;
        let content = bytes
            .get(start..end)
            .ok_or_else(|| invalid("isi record melebihi ukuran berkas"))?;
        features.push(parse_record(content)?);
        pos = end;
    }

    Ok(ShpData {
        shape_type: header.shape_type,
        bbox: header.bbox,
        features,
    })
}

/// Jumlah entri pada SHX (satu entri per record SHP).
pub fn read_shx_count(bytes: &[u8]) -> Result<usize, ShpError> {
    parse_header(bytes)?;
    let body = bytes.len() - HEADER_LEN;
    if body % RECORD_HEADER_LEN != 0 {
        return Err(invalid("panjang indeks SHX tidak kelipatan 8 byte"));
    }
    Ok(body / RECORD_HEADER_LEN)
}

struct Header {
    shape_type: i32,
    bbox: [f64; 4],
}

fn parse_header(bytes: &[u8]) -> Result<Header, ShpError> {
    if bytes.len() < HEADER_LEN {
        return Err(invalid("header kurang dari 100 byte"));
    }
    if be_i32(bytes, 0) != Some(FILE_CODE) {
        return Err(invalid("kode file bukan 9994"));
    }
    let words = be_i32(bytes, 24).unwrap_or(-1);
    if words < 0 || (words as u64) * 2 != bytes.len() as u64 {
        return Err(invalid(format!(
            "panjang di header ({} byte) tidak sama dengan ukuran berkas ({} byte)",
            (words as i64) * 2,
            bytes.len()
        )));
    }
    if le_i32(bytes, 28) != Some(VERSION) {
        return Err(invalid("versi berkas bukan 1000"));
    }
    let shape_type = le_i32(bytes, 32).unwrap_or(-1);
    let mut bbox = [0.0; 4];
    for (i, v) in bbox.iter_mut().enumerate() {
        *v = le_f64(bytes, 36 + 8 * i).unwrap_or(0.0);
    }
    Ok(Header { shape_type, bbox })
}

fn parse_record(content: &[u8]) -> Result<Vec<Vec<Coord>>, ShpError> {
    let shape = le_i32(content, 0).ok_or_else(|| invalid("record terlalu pendek"))?;
    match shape {
        0 => return Ok(Vec::new()),
        SHAPE_POLYGON => {}
        other => return Err(ShpError::UnsupportedType(other)),
    }
    let num_parts = le_i32(content, 36).ok_or_else(|| invalid("record terlalu pendek"))?;
    let num_points = le_i32(content, 40).ok_or_else(|| invalid("record terlalu pendek"))?;
    if num_parts < 0 || num_points < 0 {
        return Err(invalid("jumlah bagian atau titik negatif"));
    }
    let (np, npts) = (num_parts as usize, num_points as usize);
    let parts_at = 44usize;
    let points_at = parts_at + 4 * np;
    if (content.len() as u64) < points_at as u64 + 16 * npts as u64 {
        return Err(invalid("titik record melebihi isi"));
    }

    let mut starts = Vec::with_capacity(np);
    for i in 0..np {
        let s = le_i32(content, parts_at + 4 * i).ok_or_else(|| invalid("indeks bagian terpotong"))?;
        if s < 0 || s as usize >= npts {
            return Err(invalid("indeks bagian di luar jumlah titik"));
        }
        starts.push(s as usize);
    }

    let mut points = Vec::with_capacity(npts);
    for i in 0..npts {
        let at = points_at + 16 * i;
        let x = le_f64(content, at).ok_or_else(|| invalid("koordinat terpotong"))?;
        let y = le_f64(content, at + 8).ok_or_else(|| invalid("koordinat terpotong"))?;
        points.push([x, y]);
    }

    let mut parts = Vec::with_capacity(np);
    for (i, &s) in starts.iter().enumerate() {
        let e = starts.get(i + 1).copied().unwrap_or(npts);
        if e < s {
            return Err(invalid("urutan indeks bagian tidak naik"));
        }
        parts.push(points[s..e].to_vec());
    }
    Ok(parts)
}

fn polygon_content(ring: &[Coord]) -> Vec<u8> {
    let mut out = Vec::with_capacity(48 + 16 * ring.len());
    out.extend_from_slice(&SHAPE_POLYGON.to_le_bytes());
    for v in bbox_of(ring) {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&1i32.to_le_bytes()); // NumParts
    out.extend_from_slice(&(ring.len() as i32).to_le_bytes()); // NumPoints
    out.extend_from_slice(&0i32.to_le_bytes()); // Parts[0]
    for &[x, y] in ring {
        out.extend_from_slice(&x.to_le_bytes());
        out.extend_from_slice(&y.to_le_bytes());
    }
    out
}

fn header(file_len: usize, bbox: [f64; 4]) -> Vec<u8> {
    let mut h = Vec::with_capacity(HEADER_LEN);
    h.extend_from_slice(&FILE_CODE.to_be_bytes());
    h.extend_from_slice(&[0u8; 20]);
    h.extend_from_slice(&((file_len / 2) as i32).to_be_bytes());
    h.extend_from_slice(&VERSION.to_le_bytes());
    h.extend_from_slice(&SHAPE_POLYGON.to_le_bytes());
    for v in bbox {
        h.extend_from_slice(&v.to_le_bytes());
    }
    h.extend_from_slice(&[0u8; 32]); // Zmin, Zmax, Mmin, Mmax
    h
}

fn bbox_of(points: &[Coord]) -> [f64; 4] {
    let mut b = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
    for &[x, y] in points {
        b[0] = b[0].min(x);
        b[1] = b[1].min(y);
        b[2] = b[2].max(x);
        b[3] = b[3].max(y);
    }
    b
}

fn union_bbox(rings: &[Vec<Coord>]) -> [f64; 4] {
    let all: Vec<Coord> = rings.iter().flatten().copied().collect();
    if all.is_empty() {
        [0.0; 4]
    } else {
        bbox_of(&all)
    }
}

fn invalid(message: impl Into<String>) -> ShpError {
    ShpError::Invalid(message.into())
}

fn be_i32(b: &[u8], at: usize) -> Option<i32> {
    b.get(at..at + 4)
        .map(|s| i32::from_be_bytes(s.try_into().expect("empat byte")))
}

fn le_i32(b: &[u8], at: usize) -> Option<i32> {
    b.get(at..at + 4)
        .map(|s| i32::from_le_bytes(s.try_into().expect("empat byte")))
}

fn le_f64(b: &[u8], at: usize) -> Option<f64> {
    b.get(at..at + 8)
        .map(|s| f64::from_le_bytes(s.try_into().expect("delapan byte")))
}

#[cfg(test)]
mod tests {
    use super::*;

    const RING: [Coord; 5] = [
        [106.80, -6.20],
        [106.80, -6.19],
        [106.81, -6.19],
        [106.81, -6.20],
        [106.80, -6.20],
    ];

    #[test]
    fn writes_expected_sizes_and_round_trips() {
        let out = write_polygons(&[RING.to_vec()]).unwrap();
        // header 100 + record header 8 + content (48 + 16 * 5 titik)
        assert_eq!(out.shp.len(), 100 + 8 + 128);
        assert_eq!(out.shx.len(), 100 + 8);
        assert_eq!(read_shx_count(&out.shx).unwrap(), 1);

        let back = read_polygons(&out.shp).unwrap();
        assert_eq!(back.shape_type, SHAPE_POLYGON);
        assert_eq!(back.features.len(), 1);
        assert_eq!(back.features[0].len(), 1);
        assert_eq!(back.features[0][0], RING.to_vec());
        assert_eq!(back.bbox, [106.80, -6.20, 106.81, -6.19]);
    }

    #[test]
    fn rejects_counter_clockwise_and_open_rings() {
        let mut ccw = RING;
        ccw.reverse();
        assert_eq!(write_polygons(&[ccw.to_vec()]), Err(ShpError::NotClockwise));
        assert_eq!(
            write_polygons(&[RING[..4].to_vec()]),
            Err(ShpError::RingNotClosed)
        );
    }

    #[test]
    fn detects_corrupt_header_and_length() {
        let mut out = write_polygons(&[RING.to_vec()]).unwrap().shp;
        out.truncate(out.len() - 4);
        assert!(matches!(read_polygons(&out), Err(ShpError::Invalid(_))));
        assert!(matches!(read_polygons(&[0u8; 50]), Err(ShpError::Invalid(_))));
    }
}
