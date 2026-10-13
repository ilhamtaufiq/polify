//! Uji impor: GeoJSON dan ZIP SHP (round-trip dari hasil ekspor Polify).

use polify_core::export::{export_zip, Atribut, ExportRequest};
use polify_core::import::{from_geojson, from_shp_zip, ImportError};
use polify_core::{Coord, PortalsConfig};

fn square() -> Vec<Coord> {
    vec![
        [106.80, -6.20],
        [106.80, -6.19],
        [106.81, -6.19],
        [106.81, -6.20],
        [106.80, -6.20],
    ]
}

#[test]
fn geojson_polygon_is_imported_as_outer_ring() {
    let text = r#"{"type":"Polygon","coordinates":[[[106.8,-6.2],[106.8,-6.19],[106.81,-6.19],[106.8,-6.2]]]}"#;
    let out = from_geojson(text).unwrap();
    assert_eq!(out.features.len(), 1);
    assert_eq!(out.features[0][0], [106.8, -6.2]);
    assert!(out.warnings.is_empty());
}

#[test]
fn geojson_feature_collection_skips_points_and_warns() {
    let text = r#"{"type":"FeatureCollection","features":[
        {"type":"Feature","geometry":{"type":"Point","coordinates":[106.8,-6.2]},"properties":{}},
        {"type":"Feature","geometry":{"type":"Polygon","coordinates":[[[106.8,-6.2],[106.8,-6.19],[106.81,-6.19],[106.8,-6.2]]]},"properties":{}}
    ]}"#;
    let out = from_geojson(text).unwrap();
    assert_eq!(out.features.len(), 1);
    assert_eq!(out.warnings.len(), 1);
    assert!(out.warnings[0].contains("Point"));
}

#[test]
fn geojson_polygon_with_hole_warns_and_keeps_outer_ring() {
    let text = r#"{"type":"Polygon","coordinates":[
        [[106.8,-6.2],[106.8,-6.19],[106.81,-6.19],[106.8,-6.2]],
        [[106.802,-6.195],[106.803,-6.195],[106.803,-6.194],[106.802,-6.195]]
    ]}"#;
    let out = from_geojson(text).unwrap();
    assert_eq!(out.features[0].len(), 4);
    assert!(out.warnings.iter().any(|w| w.contains("Lubang")));
}

#[test]
fn geojson_multipolygon_gives_one_feature_per_polygon() {
    let text = r#"{"type":"MultiPolygon","coordinates":[
        [[[106.8,-6.2],[106.8,-6.19],[106.81,-6.19],[106.8,-6.2]]],
        [[[106.9,-6.2],[106.9,-6.19],[106.91,-6.19],[106.9,-6.2]]]
    ]}"#;
    let out = from_geojson(text).unwrap();
    assert_eq!(out.features.len(), 2);
}

#[test]
fn geojson_crs_member_is_reported() {
    let text = r#"{"type":"Polygon","crs":{"type":"name"},"coordinates":[[[106.8,-6.2],[106.8,-6.19],[106.81,-6.19],[106.8,-6.2]]]}"#;
    let out = from_geojson(text).unwrap();
    assert!(out.warnings.iter().any(|w| w.contains("CRS")));
}

#[test]
fn geojson_without_polygon_is_rejected() {
    let text = r#"{"type":"Point","coordinates":[106.8,-6.2]}"#;
    assert_eq!(from_geojson(text), Err(ImportError::NoPolygon));
}

#[test]
fn geojson_invalid_json_is_rejected() {
    let err = from_geojson("{bukan json").unwrap_err();
    assert_eq!(err.code(), "GEOJSON");
}

#[test]
fn geojson_position_must_have_two_numbers() {
    let text = r#"{"type":"Polygon","coordinates":[[[106.8],[106.8,-6.19],[106.81,-6.19],[106.8,-6.2]]]}"#;
    assert!(matches!(from_geojson(text), Err(ImportError::GeoJson(_))));
}

#[test]
fn shp_zip_round_trip_returns_the_exported_ring() {
    let cfg = PortalsConfig::default_config();
    let profile = cfg.resolve("combined").unwrap();
    let req = ExportRequest {
        basename: "tapak".into(),
        ring: square(),
        atribut: Atribut {
            pemrakarsa: "Warung".into(),
            kegiatan: "Warung makan".into(),
            tahun: 2026,
            provinsi: "DKI Jakarta".into(),
            keterangan: "Jakarta".into(),
        },
    };
    let zip = export_zip(&req, &cfg, &profile).unwrap().zip;

    let out = from_shp_zip(&zip).unwrap();
    assert_eq!(out.features.len(), 1);
    assert_eq!(out.features[0].len(), square().len());
    assert!(out.warnings.is_empty());
}

#[test]
fn shp_zip_without_shp_is_rejected() {
    use std::io::{Cursor, Write};
    let mut buf = Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut buf);
        let opts = zip::write::SimpleFileOptions::default();
        zip.start_file("catatan.txt", opts).unwrap();
        zip.write_all(b"tidak ada shp").unwrap();
        zip.finish().unwrap();
    }
    assert_eq!(from_shp_zip(&buf.into_inner()), Err(ImportError::MissingShp));
}

#[test]
fn shp_zip_invalid_bytes_are_rejected() {
    let err = from_shp_zip(b"bukan zip").unwrap_err();
    assert_eq!(err.code(), "ZIP_READ");
}
