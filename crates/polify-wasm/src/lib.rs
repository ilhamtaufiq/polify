//! Antarmuka JavaScript untuk `polify-core`. Semua data lintas batas memakai JSON
//! string agar kontrak mudah diuji dan tidak bergantung pada serde-wasm-bindgen.
//!
//! Kesalahan dilempar sebagai string JSON `{"code": "...", "message": "..."}`.

use polify_core::coords::{self, ConvertInput};
use polify_core::export::{self, ExportRequest};
use polify_core::import;
use polify_core::zipcheck::{self, Expected, ZipEntry};
use polify_core::{Coord, PortalsConfig};
use serde::Serialize;
use wasm_bindgen::prelude::*;

fn js_error(code: &str, message: impl std::fmt::Display) -> JsValue {
    let body = serde_json::json!({ "code": code, "message": message.to_string() });
    JsValue::from_str(&body.to_string())
}

fn to_json<T: Serialize>(value: &T) -> Result<String, JsValue> {
    serde_json::to_string(value).map_err(|e| js_error("SERIALIZE", e))
}

fn parse_json<T: serde::de::DeserializeOwned>(text: &str) -> Result<T, JsValue> {
    serde_json::from_str(text).map_err(|e| js_error("INPUT_JSON", e))
}

fn parse_expected(text: Option<String>) -> Result<Option<Expected>, JsValue> {
    text.map(|t| parse_json::<Expected>(&t)).transpose()
}

fn read_entries(bytes: &[u8]) -> Result<Vec<ZipEntry>, JsValue> {
    zipcheck::read_zip_entries(bytes).map_err(|e| js_error("ZIP_READ", e))
}

/// Konfigurasi profil portal (sama dengan config/portal-profiles.json).
#[wasm_bindgen]
pub fn portal_config() -> Result<String, JsValue> {
    to_json(&PortalsConfig::default_config())
}

/// Validasi ring `[[lon, lat], ...]`. Mengembalikan `PolygonReport` JSON.
#[wasm_bindgen]
pub fn validate_ring(ring_json: &str) -> Result<String, JsValue> {
    let ring: Vec<Coord> = parse_json(ring_json)?;
    to_json(&polify_core::validate::validate_ring(&ring, &PortalsConfig::default_config()))
}

/// Perbaikan otomatis. Mengembalikan `{ "ring": [...], "applied": [...] }` JSON.
#[wasm_bindgen]
pub fn autofix_ring(ring_json: &str) -> Result<String, JsValue> {
    let ring: Vec<Coord> = parse_json(ring_json)?;
    to_json(&polify_core::validate::autofix_ring(&ring, &PortalsConfig::default_config()))
}

/// Konversi koordinat. Input JSON, mis. `{"mode":"dms","lat":"6°12'30\"S","lon":"106°48'22\"E"}`.
#[wasm_bindgen]
pub fn convert_coordinates(input_json: &str) -> Result<String, JsValue> {
    let input: ConvertInput = parse_json(input_json)?;
    let out = coords::convert(&input).map_err(|e| js_error("COORDINATE", e))?;
    to_json(&out)
}

/// Ekspor ZIP SHP. Input: `{"basename","ring","atribut":{...}}`. Mengembalikan byte ZIP.
#[wasm_bindgen]
pub fn export_zip(request_json: &str, profile_id: &str) -> Result<Vec<u8>, JsValue> {
    let req: ExportRequest = parse_json(request_json)?;
    let cfg = PortalsConfig::default_config();
    let profile = cfg.resolve(profile_id).map_err(|e| js_error("PROFILE", e))?;
    export::export_zip(&req, &cfg, &profile)
        .map(|out| out.zip)
        .map_err(|e| js_error(e.code(), e))
}

/// Pemeriksaan ZIP untuk satu profil. `expected_json` opsional: `{"featureCount","areaM2"}`.
#[wasm_bindgen]
pub fn check_zip(bytes: &[u8], profile_id: &str, expected_json: Option<String>) -> Result<String, JsValue> {
    let entries = read_entries(bytes)?;
    let cfg = PortalsConfig::default_config();
    let profile = cfg.resolve(profile_id).map_err(|e| js_error("PROFILE", e))?;
    let expected = parse_expected(expected_json)?;
    to_json(&zipcheck::check_entries(&entries, &profile, expected.as_ref()))
}

/// Impor GeoJSON. Mengembalikan `ImportResult` JSON: `{"features": [[[lon,lat],...]], "warnings": [...]}`.
#[wasm_bindgen]
pub fn import_geojson(text: &str) -> Result<String, JsValue> {
    let out = import::from_geojson(text).map_err(|e| js_error(e.code(), e))?;
    to_json(&out)
}

/// Impor ZIP SHP. Mengembalikan `ImportResult` JSON dengan format yang sama seperti `import_geojson`.
#[wasm_bindgen]
pub fn import_shp_zip(bytes: &[u8]) -> Result<String, JsValue> {
    let out = import::from_shp_zip(bytes).map_err(|e| js_error(e.code(), e))?;
    to_json(&out)
}

/// Badge lulus/gagal untuk semua profil di konfigurasi.
#[wasm_bindgen]
pub fn zip_badges(bytes: &[u8], expected_json: Option<String>) -> Result<String, JsValue> {
    let entries = read_entries(bytes)?;
    let expected = parse_expected(expected_json)?;
    let badges = zipcheck::portal_badges(&entries, &PortalsConfig::default_config(), expected.as_ref())
        .map_err(|e| js_error("PROFILE", e))?;
    to_json(&badges)
}
