//! Penyusunan ZIP SHP (SHP, SHX, DBF, PRJ, CPG) di root ZIP, lalu pemeriksaan ulang
//! terhadap isi ZIP yang sudah ditulis. Berkas hanya dikembalikan bila lulus pemeriksaan.

use std::io::{Cursor, Write};

use serde::Deserialize;
use thiserror::Error;

use crate::dbf::{self, DbfField, DbfValue, FieldKind};
use crate::geo::Coord;
use crate::profiles::{PortalsConfig, ResolvedProfile};
use crate::shp::{self, ShpError};
use crate::validate::{self, PolygonReport};
use crate::zipcheck::{self, CheckReport, Expected, ZipEntry};

/// Definisi WKT WGS84 (GCS_WGS_1984) dalam format ESRI.
pub const WGS84_PRJ: &str = r#"GEOGCS["GCS_WGS_1984",DATUM["D_WGS_1984",SPHEROID["WGS_1984",6378137.0,298.257223563]],PRIMEM["Greenwich",0.0],UNIT["Degree",0.0174532925199433]]"#;

const DEFAULT_BASENAME: &str = "tapak";
const MAX_BASENAME_LEN: usize = 32;

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Atribut {
    /// Nama pemrakarsa, mengisi kolom PEMRAKARSA.
    pub pemrakarsa: String,
    /// Nama kegiatan, mengisi kolom KEGIATAN.
    pub kegiatan: String,
    pub tahun: u16,
    pub provinsi: String,
    pub keterangan: String,
}

/// Nilai tetap kolom LAYER pada layout Amdalnet (juknis Amdalnet, halaman 16).
pub const LAYER_TAPAK_PROYEK: &str = "Tapak Proyek";

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    pub basename: String,
    pub ring: Vec<Coord>,
    pub atribut: Atribut,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExportOutput {
    pub zip: Vec<u8>,
    pub basename: String,
    pub file_names: Vec<String>,
    pub fixes: Vec<&'static str>,
    pub validation: PolygonReport,
    pub check: CheckReport,
}

#[derive(Debug, Error)]
pub enum ExportError {
    #[error("polygon belum valid: perbaiki error validasi terlebih dahulu")]
    Validation(Box<PolygonReport>),
    #[error("pembuatan SHP gagal: {0}")]
    Shp(#[from] ShpError),
    #[error("pembuatan DBF gagal: {0}")]
    Dbf(#[from] dbf::DbfError),
    #[error("pembuatan ZIP gagal: {0}")]
    Zip(String),
    #[error("pemeriksaan ulang ZIP gagal; berkas tidak diberikan")]
    Verification(Box<CheckReport>),
}

impl ExportError {
    pub fn code(&self) -> &'static str {
        match self {
            ExportError::Validation(_) => "POLYGON_INVALID",
            ExportError::Shp(_) => "SHP_WRITE",
            ExportError::Dbf(_) => "DBF_WRITE",
            ExportError::Zip(_) => "ZIP_WRITE",
            ExportError::Verification(_) => "ZIP_VERIFICATION",
        }
    }
}

/// Ekspor satu polygon menjadi ZIP SHP untuk profil yang dipilih.
pub fn export_zip(
    req: &ExportRequest,
    cfg: &PortalsConfig,
    profile: &ResolvedProfile,
) -> Result<ExportOutput, ExportError> {
    // Layout Amdalnet mensyaratkan nama berkas huruf kapital semua (juknis, halaman 16).
    let basename = sanitize_basename(&req.basename).to_uppercase();

    let fixed = validate::autofix_ring(&req.ring, cfg);
    let validation = validate::validate_ring(&fixed.ring, cfg);
    if !validation.ok {
        return Err(ExportError::Validation(Box::new(validation)));
    }

    let shp_out = shp::write_polygons(std::slice::from_ref(&fixed.ring))?;
    let dbf_bytes = dbf::write_dbf(&dbf_fields(), &[dbf_row(req)])?;

    let entries = vec![
        ZipEntry {
            path: format!("{basename}.shp"),
            data: shp_out.shp,
        },
        ZipEntry {
            path: format!("{basename}.shx"),
            data: shp_out.shx,
        },
        ZipEntry {
            path: format!("{basename}.dbf"),
            data: dbf_bytes,
        },
        ZipEntry {
            path: format!("{basename}.prj"),
            data: WGS84_PRJ.as_bytes().to_vec(),
        },
        ZipEntry {
            path: format!("{basename}.cpg"),
            data: b"UTF-8".to_vec(),
        },
    ];
    let file_names = entries.iter().map(|e| e.path.clone()).collect();

    let zip_bytes = pack(&entries)?;
    // Pemeriksaan ulang terhadap isi ZIP yang sebenarnya, bukan data sebelum dikemas.
    let written = zipcheck::read_zip_entries(&zip_bytes).map_err(|e| ExportError::Zip(e.to_string()))?;
    let check = zipcheck::check_entries(
        &written,
        profile,
        Some(&Expected {
            feature_count: 1,
            area_m2: validation.area_m2,
        }),
    );
    if !check.ok {
        return Err(ExportError::Verification(Box::new(check)));
    }

    Ok(ExportOutput {
        zip: zip_bytes,
        basename,
        file_names,
        fixes: fixed.applied,
        validation,
        check,
    })
}

/// Nama dasar aman: huruf, angka, '_' dan '-'. Spasi menjadi '_'.
pub fn sanitize_basename(raw: &str) -> String {
    let cleaned: String = raw
        .trim()
        .chars()
        .map(|c| if c.is_whitespace() { '_' } else { c })
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .take(MAX_BASENAME_LEN)
        .collect();
    if cleaned.is_empty() {
        DEFAULT_BASENAME.to_string()
    } else {
        cleaned
    }
}

/// Layout tabel atribut yang sama untuk OSS RBA dan Amdalnet: id, PEMRAKARSA, KEGIATAN,
/// TAHUN, PROVINSI, KETERANGAN, LAYER. Semua teks kecuali id, sesuai juknis Amdalnet.
fn dbf_fields() -> Vec<DbfField> {
    let text = |name: &str| DbfField {
        name: name.into(),
        kind: FieldKind::Character,
        length: 255,
    };
    vec![
        DbfField {
            name: "ID".into(),
            kind: FieldKind::Numeric { decimals: 0 },
            length: 10,
        },
        text("PEMRAKARSA"),
        text("KEGIATAN"),
        text("TAHUN"),
        text("PROVINSI"),
        text("KETERANGAN"),
        text("LAYER"),
    ]
}

fn dbf_row(req: &ExportRequest) -> Vec<DbfValue> {
    vec![
        DbfValue::Number(1.0),
        DbfValue::Text(req.atribut.pemrakarsa.clone()),
        DbfValue::Text(req.atribut.kegiatan.clone()),
        DbfValue::Text(req.atribut.tahun.to_string()),
        DbfValue::Text(req.atribut.provinsi.clone()),
        DbfValue::Text(req.atribut.keterangan.clone()),
        DbfValue::Text(LAYER_TAPAK_PROYEK.into()),
    ]
}

fn pack(entries: &[ZipEntry]) -> Result<Vec<u8>, ExportError> {
    let zip_err = |e: &dyn std::fmt::Display| ExportError::Zip(e.to_string());
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for entry in entries {
        writer
            .start_file(entry.path.as_str(), options)
            .map_err(|e| zip_err(&e))?;
        writer.write_all(&entry.data).map_err(|e| zip_err(&e))?;
    }
    let cursor = writer.finish().map_err(|e| zip_err(&e))?;
    Ok(cursor.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ring() -> Vec<Coord> {
        vec![
            [106.80, -6.20],
            [106.80, -6.19],
            [106.81, -6.19],
            [106.81, -6.20],
        ]
    }

    fn request(ring: Vec<Coord>) -> ExportRequest {
        ExportRequest {
            basename: "Tapak Usaha 1".into(),
            ring,
            atribut: Atribut {
                pemrakarsa: "Warung Ilham".into(),
                kegiatan: "Warung makan".into(),
                tahun: 2026,
                provinsi: "DKI Jakarta".into(),
                keterangan: "Jl. Contoh No. 1, Jakarta".into(),
            },
        }
    }

    #[test]
    fn sanitize_basename_keeps_safe_characters_only() {
        assert_eq!(sanitize_basename("Tapak Usaha 1"), "Tapak_Usaha_1");
        assert_eq!(sanitize_basename("../../etc/passwd"), "etcpasswd");
        assert_eq!(sanitize_basename("   "), "tapak");
    }

    #[test]
    fn exports_verified_zip_for_combined_profile() {
        let cfg = PortalsConfig::default_config();
        let profile = cfg.resolve("combined").unwrap();
        let out = export_zip(&request(ring()), &cfg, &profile).unwrap();
        assert_eq!(out.basename, "TAPAK_USAHA_1");
        assert_eq!(
            out.file_names,
            ["TAPAK_USAHA_1.shp", "TAPAK_USAHA_1.shx", "TAPAK_USAHA_1.dbf", "TAPAK_USAHA_1.prj", "TAPAK_USAHA_1.cpg"]
        );
        assert!(out.check.ok, "{:?}", out.check.issues);
        assert_eq!(out.check.feature_count, Some(1));
    }

    #[test]
    fn clockwise_fix_is_applied_before_writing() {
        let cfg = PortalsConfig::default_config();
        let profile = cfg.resolve("combined").unwrap();
        let mut ccw = ring();
        ccw.reverse();
        let out = export_zip(&request(ccw), &cfg, &profile).unwrap();
        assert!(out.fixes.contains(&"ORIENTATION_CCW"));
    }

    #[test]
    fn invalid_polygon_is_not_exported() {
        let cfg = PortalsConfig::default_config();
        let profile = cfg.resolve("combined").unwrap();
        let bow = vec![[106.80, -6.20], [106.81, -6.19], [106.81, -6.20], [106.80, -6.19]];
        let err = export_zip(&request(bow), &cfg, &profile).unwrap_err();
        assert_eq!(err.code(), "POLYGON_INVALID");
    }
}
