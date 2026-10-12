//! Validasi geometri polygon sesuai aturan Polify, plus perbaikan otomatis.
//!
//! Setiap temuan memiliki kode stabil (untuk UI dan API), tingkat keparahan,
//! pesan berbahasa Indonesia, dan penanda apakah bisa diperbaiki otomatis.

use serde::Serialize;

use crate::geo::{self, Coord};
use crate::profiles::{Bounds, PortalsConfig};

/// Vertex dengan sudut di bawah nilai ini (derajat) dianggap duri/sliver.
const SPIKE_ANGLE_DEG: f64 = 1.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Issue {
    pub code: &'static str,
    pub severity: Severity,
    pub message: String,
    pub fixable: bool,
}

impl Issue {
    pub fn new(
        code: &'static str,
        severity: Severity,
        message: impl Into<String>,
        fixable: bool,
    ) -> Self {
        Self {
            code,
            severity,
            message: message.into(),
            fixable,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PolygonReport {
    pub ok: bool,
    pub issues: Vec<Issue>,
    pub vertex_count: usize,
    pub area_m2: f64,
    pub area_ha: f64,
    pub centroid: Option<Coord>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FixResult {
    pub ring: Vec<Coord>,
    pub applied: Vec<&'static str>,
}

/// Validasi satu ring polygon `[lon, lat]`. `ok` bernilai false bila ada error.
pub fn validate_ring(ring: &[Coord], cfg: &PortalsConfig) -> PolygonReport {
    let mut issues = Vec::new();
    if has_non_finite(ring) {
        issues.push(Issue::new(
            "NON_FINITE",
            Severity::Error,
            "Ada koordinat yang bukan angka.",
            false,
        ));
        return report(issues, ring.len(), &[]);
    }

    let bounds = &cfg.wilayah_indonesia;
    let swapped = is_swapped(ring, bounds);
    let work = if swapped {
        swap_axes(ring)
    } else {
        ring.to_vec()
    };
    if swapped {
        issues.push(Issue::new(
            "SWAPPED_LATLON",
            Severity::Error,
            "Urutan lintang dan bujur tampaknya tertukar. Gunakan 'Perbaiki otomatis'.",
            true,
        ));
    } else {
        let outside = work.iter().copied().filter(|c| !bounds.contains(*c)).count();
        if outside > 0 {
            issues.push(Issue::new(
                "OUT_OF_INDONESIA",
                Severity::Error,
                format!("{outside} titik berada di luar wilayah Indonesia."),
                false,
            ));
        }
    }

    if !work.is_empty() && !is_closed(&work) {
        issues.push(Issue::new(
            "RING_OPEN",
            Severity::Error,
            "Polygon belum tertutup (titik awal dan akhir berbeda). Akan ditutup otomatis.",
            true,
        ));
    }

    let open = geo::strip_closing(&work);
    let clean = dedupe_consecutive(open);
    if clean.len() != open.len() {
        issues.push(Issue::new(
            "DUPLICATE_VERTEX",
            Severity::Warning,
            format!(
                "Ada {} titik duplikat berurutan. Akan dihapus otomatis.",
                open.len() - clean.len()
            ),
            true,
        ));
    }
    if clean.len() < 3 {
        issues.push(Issue::new(
            "MIN_VERTICES",
            Severity::Error,
            "Polygon membutuhkan minimal 3 titik unik.",
            false,
        ));
        return report(issues, clean.len(), &clean);
    }

    if find_spike(&clean).is_some() {
        issues.push(Issue::new(
            "SPIKE",
            Severity::Warning,
            "Ada titik duri atau sliver (sudut sangat lancip). Perbaiki otomatis akan menghapus titik tersebut.",
            true,
        ));
    }
    if has_self_intersection(&clean) {
        issues.push(Issue::new(
            "SELF_INTERSECTION",
            Severity::Error,
            "Garis polygon berpotongan sendiri (bow-tie). Gambar ulang dengan urutan titik yang benar.",
            false,
        ));
    }
    if geo::signed_area_planar(&clean) > 0.0 {
        issues.push(Issue::new(
            "ORIENTATION_CCW",
            Severity::Warning,
            "Arah ring berlawanan jarum jam. Standar ESRI untuk ring luar searah jarum jam; akan dibalik otomatis.",
            true,
        ));
    }

    let area_m2 = geo::ring_area_m2(&clean);
    if area_m2 < cfg.heuristik.min_area_m2 {
        issues.push(Issue::new(
            "AREA_TOO_SMALL",
            Severity::Error,
            format!("Luas polygon terlalu kecil ({area_m2:.2} m²)."),
            false,
        ));
    } else if area_m2 / 10_000.0 > cfg.heuristik.area_warn_ha {
        issues.push(Issue::new(
            "AREA_LARGE",
            Severity::Warning,
            format!(
                "Luas {:.2} ha tidak biasa untuk satu tapak. Periksa kembali titik-titiknya.",
                area_m2 / 10_000.0
            ),
            false,
        ));
    }

    report(issues, clean.len(), &clean)
}

/// Perbaikan otomatis: tukar sumbu, tutup ring, hapus duplikat dan duri,
/// lalu balik ke arah searah jarum jam. Hasil selalu berbentuk ring tertutup.
pub fn autofix_ring(ring: &[Coord], cfg: &PortalsConfig) -> FixResult {
    let mut applied = Vec::new();
    if has_non_finite(ring) {
        return FixResult {
            ring: ring.to_vec(),
            applied,
        };
    }

    let work = if is_swapped(ring, &cfg.wilayah_indonesia) {
        applied.push("SWAPPED_LATLON");
        swap_axes(ring)
    } else {
        ring.to_vec()
    };
    if !work.is_empty() && !is_closed(&work) {
        applied.push("RING_OPEN");
    }

    let open = geo::strip_closing(&work).to_vec();
    let mut clean = dedupe_consecutive(&open);
    if clean.len() != open.len() {
        applied.push("DUPLICATE_VERTEX");
    }

    let mut spikes_removed = false;
    while clean.len() > 3 {
        match find_spike(&clean) {
            Some(i) => {
                clean.remove(i);
                spikes_removed = true;
            }
            None => break,
        }
    }
    if spikes_removed {
        applied.push("SPIKE");
    }

    if clean.len() >= 3 && geo::signed_area_planar(&clean) > 0.0 {
        clean.reverse();
        applied.push("ORIENTATION_CCW");
    }

    if let Some(&first) = clean.first() {
        clean.push(first);
    }
    FixResult {
        ring: clean,
        applied,
    }
}

/// Pemeriksaan antar-polygon: pasangan yang saling tumpang tindih.
/// Polygon yang hanya bersentuhan di batas tidak dihitung sebagai tumpang tindih.
pub fn check_overlaps(features: &[Vec<Coord>]) -> Vec<Issue> {
    let mut out = Vec::new();
    for i in 0..features.len() {
        for j in (i + 1)..features.len() {
            if polygons_overlap(
                geo::strip_closing(&features[i]),
                geo::strip_closing(&features[j]),
            ) {
                out.push(Issue::new(
                    "OVERLAP",
                    Severity::Warning,
                    format!("Polygon {} dan {} saling tumpang tindih.", i + 1, j + 1),
                    false,
                ));
            }
        }
    }
    out
}

fn report(issues: Vec<Issue>, vertex_count: usize, ring: &[Coord]) -> PolygonReport {
    let area_m2 = geo::ring_area_m2(ring);
    PolygonReport {
        ok: !issues.iter().any(|i| i.severity == Severity::Error),
        issues,
        vertex_count,
        area_m2,
        area_ha: area_m2 / 10_000.0,
        centroid: geo::centroid(ring),
    }
}

fn has_non_finite(ring: &[Coord]) -> bool {
    ring.iter().any(|c| !c[0].is_finite() || !c[1].is_finite())
}

fn is_closed(ring: &[Coord]) -> bool {
    ring.len() > 1 && ring.first() == ring.last()
}

fn swap_axes(ring: &[Coord]) -> Vec<Coord> {
    ring.iter().map(|c| [c[1], c[0]]).collect()
}

/// Semua titik berada di dalam kotak Indonesia bila sumbunya ditukar, tetapi tidak sebaliknya.
fn is_swapped(ring: &[Coord], bounds: &Bounds) -> bool {
    !ring.is_empty()
        && ring
            .iter()
            .all(|c| !bounds.contains(*c) && bounds.contains([c[1], c[0]]))
}

fn dedupe_consecutive(ring: &[Coord]) -> Vec<Coord> {
    let mut out: Vec<Coord> = Vec::with_capacity(ring.len());
    for &c in ring {
        if out.last() != Some(&c) {
            out.push(c);
        }
    }
    out
}

fn find_spike(ring: &[Coord]) -> Option<usize> {
    let n = ring.len();
    if n < 4 {
        return None;
    }
    (0..n).find(|&i| is_spike(ring[(i + n - 1) % n], ring[i], ring[(i + 1) % n]))
}

fn is_spike(a: Coord, b: Coord, c: Coord) -> bool {
    let (ux, uy) = (a[0] - b[0], a[1] - b[1]);
    let (vx, vy) = (c[0] - b[0], c[1] - b[1]);
    let lu = (ux * ux + uy * uy).sqrt();
    let lv = (vx * vx + vy * vy).sqrt();
    if lu == 0.0 || lv == 0.0 {
        return false;
    }
    let cos = (ux * vx + uy * vy) / (lu * lv);
    cos > SPIKE_ANGLE_DEG.to_radians().cos()
}

fn has_self_intersection(ring: &[Coord]) -> bool {
    let n = ring.len();
    for i in 0..n {
        let (a1, a2) = (ring[i], ring[(i + 1) % n]);
        for j in (i + 1)..n {
            if j == i + 1 || (i == 0 && j == n - 1) {
                continue;
            }
            let (b1, b2) = (ring[j], ring[(j + 1) % n]);
            if geo::segments_intersect(a1, a2, b1, b2) {
                return true;
            }
        }
    }
    false
}

fn polygons_overlap(a: &[Coord], b: &[Coord]) -> bool {
    let (na, nb) = (a.len(), b.len());
    if na < 3 || nb < 3 {
        return false;
    }
    for i in 0..na {
        let (a1, a2) = (a[i], a[(i + 1) % na]);
        for j in 0..nb {
            if geo::proper_intersect(a1, a2, b[j], b[(j + 1) % nb]) {
                return true;
            }
        }
    }
    a.iter().any(|&p| geo::point_strictly_inside(p, b))
        || b.iter().any(|&p| geo::point_strictly_inside(p, a))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CW_SQUARE: [Coord; 5] = [
        [106.80, -6.20],
        [106.80, -6.19],
        [106.81, -6.19],
        [106.81, -6.20],
        [106.80, -6.20],
    ];

    fn cfg() -> PortalsConfig {
        PortalsConfig::default_config()
    }

    fn codes(r: &PolygonReport) -> Vec<&'static str> {
        r.issues.iter().map(|i| i.code).collect()
    }

    #[test]
    fn clean_square_is_valid_with_expected_area() {
        let r = validate_ring(&CW_SQUARE, &cfg());
        assert!(r.ok, "{:?}", r.issues);
        assert!(r.issues.is_empty());
        assert!((r.area_ha - 122.4).abs() < 1.0, "{}", r.area_ha);
    }

    #[test]
    fn bow_tie_fails_without_autofix() {
        let bow = [
            [106.80, -6.20],
            [106.81, -6.19],
            [106.81, -6.20],
            [106.80, -6.19],
            [106.80, -6.20],
        ];
        let r = validate_ring(&bow, &cfg());
        assert!(!r.ok);
        assert!(codes(&r).contains(&"SELF_INTERSECTION"));
        // Bow-tie sengaja tidak diperbaiki otomatis: urutan titik harus dipilih pengguna.
        let fixed = autofix_ring(&bow, &cfg());
        assert!(!fixed.applied.iter().any(|a| *a == "SELF_INTERSECTION"));
        assert!(codes(&validate_ring(&fixed.ring, &cfg())).contains(&"SELF_INTERSECTION"));
    }

    #[test]
    fn open_ring_is_flagged_and_autofix_closes_it() {
        let open = &CW_SQUARE[..4];
        let r = validate_ring(open, &cfg());
        assert!(codes(&r).contains(&"RING_OPEN"));
        let fixed = autofix_ring(open, &cfg());
        assert!(fixed.applied.contains(&"RING_OPEN"));
        assert_eq!(fixed.ring.first(), fixed.ring.last());
        assert!(validate_ring(&fixed.ring, &cfg()).ok);
    }

    #[test]
    fn swapped_lat_lon_is_detected_and_fixed() {
        // Urutan [lintang, bujur] tertukar, dan ring belum tertutup.
        let swapped = [[-6.20, 106.80], [-6.19, 106.80], [-6.19, 106.81], [-6.20, 106.81]];
        let r = validate_ring(&swapped, &cfg());
        assert!(!r.ok);
        assert!(codes(&r).contains(&"SWAPPED_LATLON"));
        let fixed = autofix_ring(&swapped, &cfg());
        assert!(fixed.applied.contains(&"SWAPPED_LATLON"));
        assert!(validate_ring(&fixed.ring, &cfg()).ok);
    }

    #[test]
    fn coordinates_outside_indonesia_are_errors_not_autofixed() {
        let far = [[10.0, 50.0], [10.0, 50.1], [10.1, 50.1], [10.0, 50.0]];
        let r = validate_ring(&far, &cfg());
        assert!(!r.ok);
        assert!(codes(&r).contains(&"OUT_OF_INDONESIA"));
    }

    #[test]
    fn counter_clockwise_is_warning_and_autofix_reverses() {
        let mut ccw = CW_SQUARE;
        ccw.reverse();
        let r = validate_ring(&ccw, &cfg());
        assert!(r.ok, "peringatan saja, tidak memblokir");
        assert!(codes(&r).contains(&"ORIENTATION_CCW"));
        let fixed = autofix_ring(&ccw, &cfg());
        assert!(fixed.applied.contains(&"ORIENTATION_CCW"));
        assert!(geo::signed_area_planar(&fixed.ring) < 0.0);
    }

    #[test]
    fn duplicate_consecutive_vertices_are_removed() {
        let dup = [
            [106.80, -6.20],
            [106.80, -6.20],
            [106.80, -6.19],
            [106.81, -6.19],
            [106.81, -6.20],
        ];
        let r = validate_ring(&dup, &cfg());
        assert!(codes(&r).contains(&"DUPLICATE_VERTEX"));
        let fixed = autofix_ring(&dup, &cfg());
        assert_eq!(fixed.ring.len(), 5);
    }

    #[test]
    fn sharp_spike_is_warning_and_fixable() {
        // T adalah ujung sangat lancip: sudutnya ~0,14 derajat, tidak berpotongan dengan sisi lain.
        let spike = [
            [106.80, -6.20],
            [106.85, -6.20],
            [106.84, -6.2001],
            [106.84, -6.21],
            [106.80, -6.21],
        ];
        let r = validate_ring(&spike, &cfg());
        assert!(codes(&r).contains(&"SPIKE"));
        assert!(!codes(&r).contains(&"SELF_INTERSECTION"));
        let fixed = autofix_ring(&spike, &cfg());
        assert!(fixed.applied.contains(&"SPIKE"));
    }

    #[test]
    fn fewer_than_three_unique_points_fails() {
        let two = [[106.80, -6.20], [106.81, -6.19]];
        let r = validate_ring(&two, &cfg());
        assert!(codes(&r).contains(&"MIN_VERTICES"));
    }

    #[test]
    fn overlapping_polygons_are_reported_but_touching_ones_are_not() {
        let a = CW_SQUARE.to_vec();
        let b: Vec<Coord> = vec![
            [106.805, -6.195],
            [106.805, -6.185],
            [106.815, -6.185],
            [106.815, -6.195],
            [106.805, -6.195],
        ];
        assert_eq!(check_overlaps(&[a.clone(), b]).len(), 1);

        // Dua kotak bersisian (berbagi sisi di bujur 106,81) tidak tumpang tindih.
        let c: Vec<Coord> = vec![
            [106.81, -6.20],
            [106.81, -6.19],
            [106.82, -6.19],
            [106.82, -6.20],
            [106.81, -6.20],
        ];
        assert!(check_overlaps(&[a, c]).is_empty());
    }
}
