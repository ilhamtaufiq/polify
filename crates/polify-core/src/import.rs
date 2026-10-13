//! Impor poligon dari GeoJSON dan ZIP SHP. Hasilnya ring luar `[lon, lat]` per fitur,
//! lengkap dengan catatan untuk hal yang diabaikan (lubang, bagian ganda, CRS lain).

use serde::Serialize;
use serde_json::Value;
use thiserror::Error;

use crate::geo::Coord;
use crate::shp;
use crate::zipcheck;

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub features: Vec<Vec<Coord>>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Error, PartialEq)]
pub enum ImportError {
    #[error("GeoJSON tidak valid: {0}")]
    GeoJson(String),
    #[error("tidak ada poligon yang bisa diimpor")]
    NoPolygon,
    #[error("ZIP tidak dapat dibaca: {0}")]
    Zip(String),
    #[error("berkas .shp tidak ditemukan di dalam ZIP")]
    MissingShp,
    #[error("SHP tidak valid: {0}")]
    Shp(String),
}

impl ImportError {
    pub fn code(&self) -> &'static str {
        match self {
            ImportError::GeoJson(_) => "GEOJSON",
            ImportError::NoPolygon => "NO_POLYGON",
            ImportError::Zip(_) => "ZIP_READ",
            ImportError::MissingShp => "MISSING_SHP",
            ImportError::Shp(_) => "SHP_READ",
        }
    }
}

/// Impor dari teks GeoJSON (FeatureCollection, Feature, Polygon, MultiPolygon, GeometryCollection).
pub fn from_geojson(text: &str) -> Result<ImportResult, ImportError> {
    let root: Value = serde_json::from_str(text).map_err(|e| ImportError::GeoJson(e.to_string()))?;
    let mut out = ImportResult::default();
    if root.get("crs").is_some() {
        out.warnings.push("CRS pada GeoJSON diabaikan, koordinat dianggap WGS84 (EPSG:4326).".into());
    }
    walk(&root, &mut out)?;
    if out.features.is_empty() {
        return Err(ImportError::NoPolygon);
    }
    Ok(out)
}

fn walk(value: &Value, out: &mut ImportResult) -> Result<(), ImportError> {
    let kind = value
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| ImportError::GeoJson("properti \"type\" tidak ada".into()))?;
    match kind {
        "FeatureCollection" => {
            for feature in array(value, "features")? {
                walk(feature, out)?;
            }
        }
        "Feature" => match value.get("geometry") {
            Some(geometry) if !geometry.is_null() => walk(geometry, out)?,
            _ => {}
        },
        "GeometryCollection" => {
            for geometry in array(value, "geometries")? {
                walk(geometry, out)?;
            }
        }
        "Polygon" => push_polygon(array(value, "coordinates")?, out)?,
        "MultiPolygon" => {
            for polygon in array(value, "coordinates")? {
                let rings = polygon
                    .as_array()
                    .ok_or_else(|| ImportError::GeoJson("poligon harus berupa array".into()))?;
                push_polygon(rings, out)?;
            }
        }
        other => out.warnings.push(format!("geometri \"{other}\" dilewati (hanya poligon yang diimpor).")),
    }
    Ok(())
}

fn push_polygon(rings: &[Value], out: &mut ImportResult) -> Result<(), ImportError> {
    let Some(outer) = rings.first() else {
        return Err(ImportError::GeoJson("poligon tanpa ring".into()));
    };
    if rings.len() > 1 {
        out.warnings.push("Lubang (hole) pada poligon diabaikan.".into());
    }
    out.features.push(parse_ring(outer)?);
    Ok(())
}

fn parse_ring(value: &Value) -> Result<Vec<Coord>, ImportError> {
    let positions = value
        .as_array()
        .ok_or_else(|| ImportError::GeoJson("ring harus berupa array".into()))?;
    positions
        .iter()
        .map(|p| {
            let pair = p
                .as_array()
                .filter(|a| a.len() >= 2)
                .ok_or_else(|| ImportError::GeoJson("posisi harus [lon, lat]".into()))?;
            let lon = pair[0].as_f64().ok_or_else(|| ImportError::GeoJson("lon bukan angka".into()))?;
            let lat = pair[1].as_f64().ok_or_else(|| ImportError::GeoJson("lat bukan angka".into()))?;
            Ok([lon, lat])
        })
        .collect()
}

fn array<'a>(value: &'a Value, key: &str) -> Result<&'a Vec<Value>, ImportError> {
    value
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| ImportError::GeoJson(format!("properti \"{key}\" harus berupa array")))
}

/// Impor dari ZIP SHP. Membaca berkas .shp pertama; ring luar (bagian pertama) tiap fitur diambil.
pub fn from_shp_zip(bytes: &[u8]) -> Result<ImportResult, ImportError> {
    let entries = zipcheck::read_zip_entries(bytes).map_err(|e| ImportError::Zip(e.to_string()))?;
    let shp_files: Vec<_> = entries
        .iter()
        .filter(|e| e.path.to_ascii_lowercase().ends_with(".shp"))
        .collect();
    let first = shp_files.first().ok_or(ImportError::MissingShp)?;

    let mut out = ImportResult::default();
    if shp_files.len() > 1 {
        out.warnings.push(format!(
            "ZIP berisi {} berkas .shp; hanya \"{}\" yang dibaca.",
            shp_files.len(),
            first.path
        ));
    }

    let data = shp::read_polygons(&first.data).map_err(|e| ImportError::Shp(e.to_string()))?;
    let mut multi_part = false;
    for parts in data.features {
        if parts.len() > 1 {
            multi_part = true;
        }
        if let Some(outer) = parts.into_iter().next() {
            out.features.push(outer);
        }
    }
    if multi_part {
        out.warnings.push("Bagian ganda pada fitur SHP diabaikan (hanya ring luar pertama).".into());
    }
    if out.features.is_empty() {
        return Err(ImportError::NoPolygon);
    }
    Ok(out)
}
