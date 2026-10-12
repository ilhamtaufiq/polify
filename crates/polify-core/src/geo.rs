//! Primitif geometri datar untuk polygon.
//!
//! Koordinat disimpan sebagai `[lon, lat]` dalam derajat WGS84 (x = bujur, y = lintang).

use crate::utm;

/// Satu koordinat `[lon, lat]` dalam derajat WGS84.
pub type Coord = [f64; 2];

/// Buang titik penutup bila ring sudah tertutup (titik pertama sama dengan terakhir).
pub fn strip_closing(ring: &[Coord]) -> &[Coord] {
    match (ring.first(), ring.last()) {
        (Some(first), Some(last)) if ring.len() > 1 && first == last => &ring[..ring.len() - 1],
        _ => ring,
    }
}

/// Luas bertanda rumus shoelace pada bidang datar.
/// Nilai negatif berarti searah jarum jam (standar ESRI untuk ring luar).
pub fn signed_area_planar(ring: &[Coord]) -> f64 {
    let n = ring.len();
    if n < 3 {
        return 0.0;
    }
    let sum: f64 = (0..n)
        .map(|i| {
            let [x1, y1] = ring[i];
            let [x2, y2] = ring[(i + 1) % n];
            x1 * y2 - x2 * y1
        })
        .sum();
    sum / 2.0
}

/// Luas (m²) polygon di atas elipsoid WGS84, dihitung pada proyeksi UTM
/// zona yang paling dekat dengan pusat polygon. Hasil selalu positif.
pub fn ring_area_m2(ring: &[Coord]) -> f64 {
    let open = strip_closing(ring);
    if open.len() < 3 {
        return 0.0;
    }
    let mean_lon = open.iter().map(|c| c[0]).sum::<f64>() / open.len() as f64;
    let lon0 = utm::central_meridian(utm::zone_for_lon(mean_lon));
    let projected: Vec<Coord> = open
        .iter()
        .map(|c| {
            let (e, n) = utm::project_raw(c[0], c[1], lon0);
            [e, n]
        })
        .collect();
    signed_area_planar(&projected).abs()
}

/// Rata-rata vertex dari ring terbuka (titik pusat sederhana untuk pelaporan).
pub fn centroid(ring: &[Coord]) -> Option<Coord> {
    let open = strip_closing(ring);
    if open.is_empty() {
        return None;
    }
    let n = open.len() as f64;
    let (sx, sy) = open
        .iter()
        .fold((0.0, 0.0), |(sx, sy), c| (sx + c[0], sy + c[1]));
    Some([sx / n, sy / n])
}

fn orient(a: Coord, b: Coord, c: Coord) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

fn on_segment(a: Coord, b: Coord, p: Coord) -> bool {
    p[0] >= a[0].min(b[0])
        && p[0] <= a[0].max(b[0])
        && p[1] >= a[1].min(b[1])
        && p[1] <= a[1].max(b[1])
}

/// Dua segmen berpotongan atau bersentuhan (termasuk titik ujung).
pub fn segments_intersect(p1: Coord, p2: Coord, p3: Coord, p4: Coord) -> bool {
    let d1 = orient(p3, p4, p1);
    let d2 = orient(p3, p4, p2);
    let d3 = orient(p1, p2, p3);
    let d4 = orient(p1, p2, p4);
    if ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0))
        && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0))
    {
        return true;
    }
    (d1 == 0.0 && on_segment(p3, p4, p1))
        || (d2 == 0.0 && on_segment(p3, p4, p2))
        || (d3 == 0.0 && on_segment(p1, p2, p3))
        || (d4 == 0.0 && on_segment(p1, p2, p4))
}

/// Dua segmen berpotongan secara tegas (bukan sekadar bersentuhan).
pub fn proper_intersect(p1: Coord, p2: Coord, p3: Coord, p4: Coord) -> bool {
    let d1 = orient(p3, p4, p1);
    let d2 = orient(p3, p4, p2);
    let d3 = orient(p1, p2, p3);
    let d4 = orient(p1, p2, p4);
    ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0))
        && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0))
}

/// Titik berada tepat pada garis batas ring.
pub fn on_boundary(p: Coord, ring: &[Coord]) -> bool {
    let n = ring.len();
    (0..n).any(|i| {
        let a = ring[i];
        let b = ring[(i + 1) % n];
        orient(a, b, p) == 0.0 && on_segment(a, b, p)
    })
}

/// Titik berada di dalam ring (ray casting). Titik pada batas dianggap di luar.
pub fn point_strictly_inside(p: Coord, ring: &[Coord]) -> bool {
    let n = ring.len();
    if n < 3 || on_boundary(p, ring) {
        return false;
    }
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let [xi, yi] = ring[i];
        let [xj, yj] = ring[j];
        if (yi > p[1]) != (yj > p[1]) && p[0] < (xj - xi) * (p[1] - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clockwise_square_has_negative_signed_area() {
        let cw = [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
        assert!(signed_area_planar(&cw) < 0.0);
        assert!((signed_area_planar(&cw).abs() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn area_of_small_square_is_close_to_expected() {
        // 0,01° x 0,01° di sekitar 6,2° LS ≈ 1,106 km x 1,107 km ≈ 122,4 ha
        let ring = [
            [106.80, -6.20],
            [106.80, -6.19],
            [106.81, -6.19],
            [106.81, -6.20],
            [106.80, -6.20],
        ];
        let ha = ring_area_m2(&ring) / 10_000.0;
        assert!((ha - 122.4).abs() < 1.0, "luas {ha} ha");
    }

    #[test]
    fn point_inside_and_on_boundary() {
        let sq = [[0.0, 0.0], [0.0, 2.0], [2.0, 2.0], [2.0, 0.0]];
        assert!(point_strictly_inside([1.0, 1.0], &sq));
        assert!(!point_strictly_inside([3.0, 1.0], &sq));
        assert!(!point_strictly_inside([0.0, 1.0], &sq));
    }

    #[test]
    fn proper_intersect_ignores_shared_endpoint() {
        assert!(proper_intersect([0.0, 0.0], [2.0, 2.0], [0.0, 2.0], [2.0, 0.0]));
        assert!(!proper_intersect([0.0, 0.0], [1.0, 1.0], [1.0, 1.0], [2.0, 0.0]));
    }
}
