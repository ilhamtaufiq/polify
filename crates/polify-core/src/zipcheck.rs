//! Pemeriksaan ZIP SHP: kelengkapan komponen, nama dasar seragam, CRS, encoding,
//! serta pembacaan ulang SHP/SHX/DBF untuk membandingkan jumlah fitur dan luas.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Cursor, Read};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::dbf;
use crate::geo;
use crate::profiles::{PortalsConfig, ProfileError, ResolvedProfile};
use crate::shp::{self, ShpError};
use crate::validate::{Issue, Severity};

const SHAPE_EXTENSIONS: [&str; 5] = ["shp", "shx", "dbf", "prj", "cpg"];
/// Toleransi selisih luas (rasio) antara data peta dan SHP hasil baca ulang. Heuristik internal.
const AREA_TOLERANCE_RATIO: f64 = 0.005;

#[derive(Debug, Clone, PartialEq)]
pub struct ZipEntry {
    pub path: String,
    pub data: Vec<u8>,
}

/// Nilai acuan dari data di peta, untuk dibandingkan dengan SHP hasil baca ulang.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Expected {
    pub feature_count: usize,
    pub area_m2: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CheckReport {
    pub profile: String,
    pub ok: bool,
    pub issues: Vec<Issue>,
    pub files: Vec<String>,
    pub feature_count: Option<usize>,
    pub area_m2: Option<f64>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PortalBadge {
    pub profile: String,
    pub label: String,
    pub ok: bool,
    pub errors: usize,
    pub warnings: usize,
}

#[derive(Debug, Error)]
pub enum ZipReadError {
    #[error("ZIP tidak dapat dibaca: {0}")]
    Read(String),
}

/// Baca semua berkas (bukan folder) dari sebuah ZIP ke memori.
pub fn read_zip_entries(bytes: &[u8]) -> Result<Vec<ZipEntry>, ZipReadError> {
    let read_err = |e: &dyn std::fmt::Display| ZipReadError::Read(e.to_string());
    let mut archive =
        zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| read_err(&e))?;
    let mut entries = Vec::with_capacity(archive.len());
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| read_err(&e))?;
        if file.is_dir() {
            continue;
        }
        let path = file.name().to_string();
        let mut data = Vec::new();
        file.read_to_end(&mut data).map_err(|e| read_err(&e))?;
        entries.push(ZipEntry { path, data });
    }
    Ok(entries)
}

/// Periksa isi ZIP terhadap satu profil portal.
pub fn check_entries(
    entries: &[ZipEntry],
    profile: &ResolvedProfile,
    expected: Option<&Expected>,
) -> CheckReport {
    let mut issues = Vec::new();
    let files: Vec<String> = entries.iter().map(|e| e.path.clone()).collect();
    let total_bytes: usize = entries.iter().map(|e| e.data.len()).sum();

    let mut stems = BTreeSet::new();
    let mut components: BTreeMap<String, &ZipEntry> = BTreeMap::new();
    for entry in entries {
        let path = entry.path.replace('\\', "/");
        let file_name = path.rsplit('/').next().unwrap_or(&path);
        if path.contains('/') {
            issues.push(Issue::new(
                "FILE_DI_SUBFOLDER",
                Severity::Warning,
                format!(
                    "'{}' berada di dalam subfolder. Simpan berkas SHP di root ZIP.",
                    entry.path
                ),
                false,
            ));
        }
        if !is_safe_name(file_name) {
            issues.push(Issue::new(
                "UNSAFE_NAME",
                Severity::Warning,
                format!(
                    "Nama file '{file_name}' memuat karakter yang tidak aman. Gunakan huruf, angka, '_' atau '-'."
                ),
                false,
            ));
        }

        let (stem, ext) = split_ext(&path);
        let ext = ext.to_ascii_lowercase();
        if SHAPE_EXTENSIONS.contains(&ext.as_str()) {
            stems.insert(stem.to_string());
            components.entry(ext).or_insert(entry);
        }
    }

    if stems.len() > 1 {
        issues.push(Issue::new(
            "BASENAME_MISMATCH",
            Severity::Error,
            format!(
                "Nama dasar komponen SHP tidak seragam: {}.",
                stems.iter().cloned().collect::<Vec<_>>().join(", ")
            ),
            false,
        ));
    }

    for ext in &profile.required_extensions {
        if !components.contains_key(&ext.to_ascii_lowercase()) {
            issues.push(Issue::new(
                missing_code(ext),
                Severity::Error,
                format!(
                    "Berkas .{ext} tidak ditemukan di dalam ZIP. {} mewajibkan .{ext}.",
                    profile.label
                ),
                false,
            ));
        }
    }

    if let Some(cpg) = components.get("cpg") {
        let value = String::from_utf8_lossy(strip_bom(&cpg.data)).trim().to_string();
        if normalize_encoding(&value) != normalize_encoding(&profile.encoding) {
            issues.push(Issue::new(
                "CPG_NOT_UTF8",
                Severity::Error,
                format!("Isi .cpg harus '{}', ditemukan '{value}'.", profile.encoding),
                false,
            ));
        }
    }

    if let Some(prj) = components.get("prj") {
        if profile.crs == "EPSG:4326" && !is_wgs84_wkt(&prj.data) {
            issues.push(Issue::new(
                "PRJ_NOT_WGS84",
                Severity::Error,
                "Berkas .prj tidak terbaca sebagai WGS84 (EPSG:4326). Unduh ulang dengan CRS WGS84.",
                false,
            ));
        }
    }

    let mut feature_count = None;
    let mut area_m2 = None;
    if let Some(shp_entry) = components.get("shp") {
        match shp::read_polygons(&shp_entry.data) {
            Err(ShpError::UnsupportedType(t)) => issues.push(Issue::new(
                "SHP_BUKAN_POLYGON",
                Severity::Error,
                format!("Tipe bentuk SHP {t} bukan Polygon (5)."),
                false,
            )),
            Err(e) => issues.push(Issue::new(
                "SHP_RUSAK",
                Severity::Error,
                format!("Berkas .shp tidak valid: {e}."),
                false,
            )),
            Ok(data) => {
                let count = data.features.len();
                let mut total = 0.0;
                for parts in &data.features {
                    if parts.len() > 1 {
                        issues.push(Issue::new(
                            "MULTI_PART",
                            Severity::Warning,
                            "Ada fitur dengan lebih dari satu bagian (multipart).",
                            false,
                        ));
                    }
                    if parts.first().is_some_and(|outer| geo::signed_area_planar(outer) > 0.0) {
                        issues.push(Issue::new(
                            "ORIENTATION_CCW",
                            Severity::Warning,
                            "Ring luar tidak searah jarum jam (standar ESRI).",
                            false,
                        ));
                    }
                    total += parts.iter().map(|p| geo::ring_area_m2(p)).sum::<f64>();
                }
                match count {
                    0 => issues.push(Issue::new(
                        "NO_FEATURES",
                        Severity::Error,
                        "Berkas .shp tidak memuat fitur.",
                        false,
                    )),
                    1 => {}
                    _ => issues.push(Issue::new(
                        "MULTI_FEATURE",
                        Severity::Warning,
                        format!("Berkas memuat {count} fitur; sebaiknya satu polygon per berkas."),
                        false,
                    )),
                }
                if let Some(max) = profile.max_features {
                    if count > max as usize {
                        issues.push(Issue::new(
                            "MAX_FEATURES",
                            Severity::Error,
                            format!("Jumlah fitur {count} melebihi batas {max}."),
                            false,
                        ));
                    }
                }
                if let Some(exp) = expected {
                    if count != exp.feature_count {
                        issues.push(Issue::new(
                            "FEATURE_COUNT_MISMATCH",
                            Severity::Error,
                            format!(
                                "Jumlah fitur hasil baca ulang ({count}) tidak sama dengan data di peta ({}).",
                                exp.feature_count
                            ),
                            false,
                        ));
                    }
                    let tolerance = AREA_TOLERANCE_RATIO * exp.area_m2.abs().max(1.0);
                    if (total - exp.area_m2).abs() > tolerance {
                        issues.push(Issue::new(
                            "AREA_MISMATCH",
                            Severity::Error,
                            format!(
                                "Luas hasil baca ulang ({total:.2} m²) berbeda dari data di peta ({:.2} m²).",
                                exp.area_m2
                            ),
                            false,
                        ));
                    }
                }
                feature_count = Some(count);
                area_m2 = Some(total);
            }
        }
    }

    if let Some(shx) = components.get("shx") {
        match shp::read_shx_count(&shx.data) {
            Err(e) => issues.push(Issue::new(
                "SHX_RUSAK",
                Severity::Error,
                format!("Berkas .shx tidak valid: {e}."),
                false,
            )),
            Ok(n) => {
                if let Some(count) = feature_count {
                    if n != count {
                        issues.push(Issue::new(
                            "SHX_MISMATCH",
                            Severity::Error,
                            format!("Indeks .shx memuat {n} entri, sedangkan .shp memuat {count} fitur."),
                            false,
                        ));
                    }
                }
            }
        }
    }

    if let Some(dbf_entry) = components.get("dbf") {
        match dbf::read_header(&dbf_entry.data) {
            Err(e) => issues.push(Issue::new(
                "DBF_RUSAK",
                Severity::Error,
                format!("Berkas .dbf tidak valid: {e}."),
                false,
            )),
            Ok(info) => {
                if let Some(count) = feature_count {
                    if info.record_count as usize != count {
                        issues.push(Issue::new(
                            "DBF_COUNT_MISMATCH",
                            Severity::Error,
                            format!(
                                "Atribut .dbf memuat {} baris, sedangkan .shp memuat {count} fitur.",
                                info.record_count
                            ),
                            false,
                        ));
                    }
                }
            }
        }
    }

    match profile.max_file_size_mb {
        Some(max_mb) => {
            let mb = total_bytes as f64 / 1_048_576.0;
            if mb > max_mb {
                issues.push(Issue::new(
                    "FILE_TOO_LARGE",
                    Severity::Error,
                    format!("Ukuran berkas {mb:.2} MB melebihi batas {max_mb} MB."),
                    false,
                ));
            }
        }
        None => issues.push(Issue::new(
            "SIZE_LIMIT_UNVERIFIED",
            Severity::Info,
            "Batas ukuran file untuk portal ini belum diketahui sehingga belum diverifikasi.",
            false,
        )),
    }

    CheckReport {
        profile: profile.id.clone(),
        ok: !issues.iter().any(|i| i.severity == Severity::Error),
        issues,
        files,
        feature_count,
        area_m2,
    }
}

/// Badge lulus/gagal untuk setiap profil di konfigurasi (termasuk profil gabungan).
pub fn portal_badges(
    entries: &[ZipEntry],
    cfg: &PortalsConfig,
    expected: Option<&Expected>,
) -> Result<Vec<PortalBadge>, ProfileError> {
    cfg.profile_ids()
        .into_iter()
        .map(|id| {
            let profile = cfg.resolve(&id)?;
            let report = check_entries(entries, &profile, expected);
            let count = |sev: Severity| report.issues.iter().filter(|i| i.severity == sev).count();
            Ok(PortalBadge {
                ok: report.ok,
                errors: count(Severity::Error),
                warnings: count(Severity::Warning),
                label: profile.label,
                profile: id,
            })
        })
        .collect()
}

fn missing_code(ext: &str) -> &'static str {
    match ext.to_ascii_lowercase().as_str() {
        "shp" => "MISSING_SHP",
        "shx" => "MISSING_SHX",
        "dbf" => "MISSING_DBF",
        "prj" => "MISSING_PRJ",
        "cpg" => "MISSING_CPG",
        _ => "MISSING_OTHER",
    }
}

/// Pisahkan ekstensi dari nama berkas. Titik di nama folder diabaikan.
fn split_ext(path: &str) -> (&str, &str) {
    let name_start = path.rfind('/').map_or(0, |i| i + 1);
    match path[name_start..].rfind('.') {
        Some(i) => (&path[..name_start + i], &path[name_start + i + 1..]),
        None => (path, ""),
    }
}

fn is_safe_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

fn strip_bom(data: &[u8]) -> &[u8] {
    data.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(data)
}

fn normalize_encoding(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

fn is_wgs84_wkt(data: &[u8]) -> bool {
    let up = String::from_utf8_lossy(data).to_ascii_uppercase();
    up.contains("GEOGCS") && (up.contains("WGS_1984") || up.contains("WGS84") || up.contains("WGS 84"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> ResolvedProfile {
        PortalsConfig::default_config().resolve("combined").unwrap()
    }

    #[test]
    fn split_ext_ignores_dots_in_folders() {
        assert_eq!(split_ext("a.b/tapak.shp"), ("a.b/tapak", "shp"));
        assert_eq!(split_ext("README"), ("README", ""));
    }

    #[test]
    fn detects_mixed_basenames() {
        let entries = vec![
            ZipEntry { path: "tapak.shp".into(), data: vec![] },
            ZipEntry { path: "lain.dbf".into(), data: vec![] },
        ];
        let r = check_entries(&entries, &profile(), None);
        let codes: Vec<_> = r.issues.iter().map(|i| i.code).collect();
        assert!(codes.contains(&"BASENAME_MISMATCH"));
        assert!(codes.contains(&"MISSING_SHX"));
        assert!(!r.ok);
    }

    #[test]
    fn subfolder_is_warning_not_error() {
        let entries = vec![ZipEntry { path: "data/tapak.txt".into(), data: vec![] }];
        let r = check_entries(&entries, &profile(), None);
        let sub = r.issues.iter().find(|i| i.code == "FILE_DI_SUBFOLDER").unwrap();
        assert_eq!(sub.severity, Severity::Warning);
    }
}
