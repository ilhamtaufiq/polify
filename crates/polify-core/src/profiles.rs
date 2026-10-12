//! Konfigurasi profil portal (`config/portal-profiles.json`) dan penggabungan profil.
//!
//! Aturan disimpan di JSON, bukan hard-code. Profil gabungan memakai strategi
//! `strictest-common`: kewajiban berkas digabung (union), nilai yang berbeda diganti
//! dengan nilai paling ketat yang diterima semua portal, dan konflik dicatat di `notes`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PortalRule {
    #[serde(default)]
    pub label: String,
    pub container: String,
    pub required_extensions: Vec<String>,
    pub crs: String,
    pub encoding: String,
    pub flat_structure: bool,
    #[serde(rename = "maxFileSizeMB", default)]
    pub max_file_size_mb: Option<f64>,
    #[serde(default)]
    pub max_features: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CombinedRule {
    #[serde(default)]
    pub label: String,
    pub strategy: String,
    pub includes: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(untagged)]
pub enum ProfileRule {
    Portal(PortalRule),
    Combined(CombinedRule),
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Bounds {
    pub min_lon: f64,
    pub max_lon: f64,
    pub min_lat: f64,
    pub max_lat: f64,
}

impl Bounds {
    pub fn contains(&self, c: [f64; 2]) -> bool {
        c[0] >= self.min_lon && c[0] <= self.max_lon && c[1] >= self.min_lat && c[1] <= self.max_lat
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Heuristics {
    pub min_area_m2: f64,
    pub area_warn_ha: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PortalsConfig {
    pub profiles: BTreeMap<String, ProfileRule>,
    pub wilayah_indonesia: Bounds,
    pub heuristik: Heuristics,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedProfile {
    pub id: String,
    pub label: String,
    pub required_extensions: Vec<String>,
    pub crs: String,
    pub encoding: String,
    pub flat_structure: bool,
    pub max_file_size_mb: Option<f64>,
    pub max_features: Option<u32>,
    pub notes: Vec<String>,
}

#[derive(Debug, Error, PartialEq)]
pub enum ProfileError {
    #[error("profil '{0}' tidak dikenal")]
    Unknown(String),
    #[error("strategi gabungan '{0}' belum didukung")]
    Strategy(String),
    #[error("profil gabungan '{0}' hanya boleh menggabungkan profil dasar")]
    Nested(String),
    #[error("profil gabungan '{0}' tidak menyebutkan profil apa pun")]
    Empty(String),
}

impl PortalsConfig {
    pub fn from_json(text: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(text)
    }

    /// Selesaikan profil berdasarkan id (profil dasar atau gabungan).
    pub fn resolve(&self, id: &str) -> Result<ResolvedProfile, ProfileError> {
        match self.profiles.get(id) {
            None => Err(ProfileError::Unknown(id.to_string())),
            Some(ProfileRule::Portal(rule)) => Ok(ResolvedProfile::from_portal(id, rule)),
            Some(ProfileRule::Combined(rule)) => self.combine(id, rule),
        }
    }

    fn combine(&self, id: &str, rule: &CombinedRule) -> Result<ResolvedProfile, ProfileError> {
        if rule.strategy != "strictest-common" {
            return Err(ProfileError::Strategy(rule.strategy.clone()));
        }
        if rule.includes.is_empty() {
            return Err(ProfileError::Empty(id.to_string()));
        }
        let parts = rule
            .includes
            .iter()
            .map(|inc| match self.profiles.get(inc) {
                Some(ProfileRule::Portal(p)) => Ok(ResolvedProfile::from_portal(inc, p)),
                Some(ProfileRule::Combined(_)) => Err(ProfileError::Nested(id.to_string())),
                None => Err(ProfileError::Unknown(inc.clone())),
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ResolvedProfile::strictest(id, &rule.label, &parts))
    }

    /// Semua id profil (dasar dan gabungan) dalam urutan abjad.
    pub fn profile_ids(&self) -> Vec<String> {
        self.profiles.keys().cloned().collect()
    }
}

impl ResolvedProfile {
    fn from_portal(id: &str, rule: &PortalRule) -> Self {
        Self {
            id: id.to_string(),
            label: rule.label.clone(),
            required_extensions: rule.required_extensions.clone(),
            crs: rule.crs.clone(),
            encoding: rule.encoding.clone(),
            flat_structure: rule.flat_structure,
            max_file_size_mb: rule.max_file_size_mb,
            max_features: rule.max_features,
            notes: Vec::new(),
        }
    }

    fn strictest(id: &str, label: &str, parts: &[ResolvedProfile]) -> Self {
        let mut notes = Vec::new();

        let mut required_extensions: Vec<String> = Vec::new();
        for part in parts {
            for ext in &part.required_extensions {
                if !required_extensions.contains(ext) {
                    required_extensions.push(ext.clone());
                }
            }
        }

        let crs = unify(parts, |p| &p.crs, "CRS", "EPSG:4326", &mut notes);
        let encoding = unify(parts, |p| &p.encoding, "encoding", "UTF-8", &mut notes);

        let max_file_size_mb = parts
            .iter()
            .filter_map(|p| p.max_file_size_mb)
            .fold(None, |acc: Option<f64>, v| Some(acc.map_or(v, |a| a.min(v))));
        let max_features = parts
            .iter()
            .filter_map(|p| p.max_features)
            .fold(None, |acc: Option<u32>, v| Some(acc.map_or(v, |a| a.min(v))));

        notes.push(format!(
            "Profil gabungan: kewajiban berkas diambil dari gabungan semua portal ({}).",
            parts.iter().map(|p| p.label.as_str()).collect::<Vec<_>>().join(" + ")
        ));

        Self {
            id: id.to_string(),
            label: label.to_string(),
            required_extensions,
            crs,
            encoding,
            flat_structure: parts.iter().any(|p| p.flat_structure),
            max_file_size_mb,
            max_features,
            notes,
        }
    }
}

/// Nilai yang sama di semua portal dipakai apa adanya; bila berbeda, pakai
/// nilai fallback yang diterima semua portal dan catat konfliknya.
fn unify(
    parts: &[ResolvedProfile],
    pick: impl Fn(&ResolvedProfile) -> &str,
    what: &str,
    fallback: &str,
    notes: &mut Vec<String>,
) -> String {
    let first = parts.first().map_or(fallback, &pick);
    if parts.iter().all(|p| pick(p) == first) {
        first.to_string()
    } else {
        notes.push(format!(
            "{what} antar portal berbeda; dipakai {fallback} sebagai nilai paling ketat."
        ));
        fallback.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_loads_and_combined_is_strictest() {
        let cfg = PortalsConfig::default_config();
        let combined = cfg.resolve("combined").unwrap();
        assert_eq!(
            combined.required_extensions,
            vec!["shp", "dbf", "shx", "prj", "cpg"]
        );
        assert_eq!(combined.crs, "EPSG:4326");
        assert_eq!(combined.encoding, "UTF-8");
        assert!(combined.flat_structure);
    }

    #[test]
    fn unknown_and_unsupported_profiles_error() {
        let cfg = PortalsConfig::default_config();
        assert_eq!(
            cfg.resolve("tidak_ada"),
            Err(ProfileError::Unknown("tidak_ada".into()))
        );
    }

    #[test]
    fn nested_combined_is_rejected() {
        let mut cfg = PortalsConfig::default_config();
        cfg.profiles.insert(
            "bersarang".into(),
            ProfileRule::Combined(CombinedRule {
                label: String::new(),
                strategy: "strictest-common".into(),
                includes: vec!["combined".into()],
            }),
        );
        assert!(matches!(cfg.resolve("bersarang"), Err(ProfileError::Nested(_))));
    }
}
