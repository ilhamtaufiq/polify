//! Uji integrasi: alur lengkap polygon -> validasi -> ZIP SHP -> pemeriksaan ulang.

use polify_core::export::{export_zip, Atribut, ExportRequest};
use polify_core::zipcheck::{check_entries, portal_badges, read_zip_entries, Expected, ZipEntry};
use polify_core::{Coord, PortalsConfig};

fn cfg() -> PortalsConfig {
    PortalsConfig::default_config()
}

fn square() -> Vec<Coord> {
    vec![
        [106.80, -6.20],
        [106.80, -6.19],
        [106.81, -6.19],
        [106.81, -6.20],
        [106.80, -6.20],
    ]
}

fn valid_zip() -> Vec<u8> {
    let cfg = cfg();
    let profile = cfg.resolve("combined").unwrap();
    let req = ExportRequest {
        basename: "tapak".into(),
        ring: square(),
        atribut: Atribut {
            nama: "Warung".into(),
            alamat: "Jakarta".into(),
            kode_kbli: "47111".into(),
            tahun: 2026,
        },
    };
    export_zip(&req, &cfg, &profile).unwrap().zip
}

fn without(entries: Vec<ZipEntry>, ext: &str) -> Vec<ZipEntry> {
    entries
        .into_iter()
        .filter(|e| !e.path.ends_with(ext))
        .collect()
}

fn codes(entries: &[ZipEntry], profile_id: &str, expected: Option<&Expected>) -> Vec<&'static str> {
    let cfg = cfg();
    let profile = cfg.resolve(profile_id).unwrap();
    check_entries(entries, &profile, expected)
        .issues
        .iter()
        .map(|i| i.code)
        .collect()
}

#[test]
fn exported_zip_passes_both_portals_and_matches_map_data() {
    let entries = read_zip_entries(&valid_zip()).unwrap();
    let expected = Expected {
        feature_count: 1,
        area_m2: polify_core::geo::ring_area_m2(&square()),
    };
    let cfg = cfg();
    let badges = portal_badges(&entries, &cfg, Some(&expected)).unwrap();
    assert!(badges.iter().all(|b| b.ok), "{badges:?}");
}

#[test]
fn missing_cpg_fails_amdalnet_with_specific_code() {
    let entries = without(read_zip_entries(&valid_zip()).unwrap(), ".cpg");
    assert!(codes(&entries, "amdalnet", None).contains(&"MISSING_CPG"));
}

#[test]
fn missing_shx_fails_both_portals() {
    let entries = without(read_zip_entries(&valid_zip()).unwrap(), ".shx");
    assert!(codes(&entries, "oss_rba", None).contains(&"MISSING_SHX"));
    assert!(codes(&entries, "combined", None).contains(&"MISSING_SHX"));
}

#[test]
fn missing_prj_is_flagged_for_oss_rba_too() {
    let entries = without(read_zip_entries(&valid_zip()).unwrap(), ".prj");
    assert!(codes(&entries, "oss_rba", None).contains(&"MISSING_PRJ"));
}

#[test]
fn basename_mismatch_fails() {
    let mut entries = read_zip_entries(&valid_zip()).unwrap();
    entries[1].path = "lain.shx".into();
    assert!(codes(&entries, "combined", None).contains(&"BASENAME_MISMATCH"));
}

#[test]
fn file_in_subfolder_is_only_a_warning() {
    let mut entries = read_zip_entries(&valid_zip()).unwrap();
    entries[0].path = "data/tapak.shp".into();
    let cfg = cfg();
    let profile = cfg.resolve("combined").unwrap();
    let report = check_entries(&entries, &profile, None);
    let sub = report
        .issues
        .iter()
        .find(|i| i.code == "FILE_DI_SUBFOLDER")
        .expect("peringatan subfolder");
    assert_eq!(sub.severity, polify_core::validate::Severity::Warning);
}

#[test]
fn non_utf8_cpg_and_non_wgs84_prj_fail() {
    let mut entries = read_zip_entries(&valid_zip()).unwrap();
    for e in entries.iter_mut() {
        if e.path.ends_with(".cpg") {
            e.data = b"1252".to_vec();
        }
        if e.path.ends_with(".prj") {
            e.data = br#"PROJCS["DGN_1995_UTM_48S"]"#.to_vec();
        }
    }
    let found = codes(&entries, "combined", None);
    assert!(found.contains(&"CPG_NOT_UTF8"));
    assert!(found.contains(&"PRJ_NOT_WGS84"));
}

#[test]
fn corrupt_shp_length_is_detected() {
    let mut entries = read_zip_entries(&valid_zip()).unwrap();
    let shp = entries.iter_mut().find(|e| e.path.ends_with(".shp")).unwrap();
    shp.data.truncate(shp.data.len() - 8);
    assert!(codes(&entries, "combined", None).contains(&"SHP_RUSAK"));
}

#[test]
fn area_mismatch_against_map_is_reported() {
    let entries = read_zip_entries(&valid_zip()).unwrap();
    let wrong = Expected {
        feature_count: 1,
        area_m2: 10.0,
    };
    assert!(codes(&entries, "combined", Some(&wrong)).contains(&"AREA_MISMATCH"));
}

#[test]
fn zip_round_trip_is_stable_across_reads() {
    let first = read_zip_entries(&valid_zip()).unwrap();
    let second = read_zip_entries(&valid_zip()).unwrap();
    assert_eq!(first, second, "ekspor deterministik");
}
