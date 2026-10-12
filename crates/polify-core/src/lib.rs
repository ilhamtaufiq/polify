//! Inti logika geospasial Polify.
//!
//! Crate ini dipakai dua cara: dikompilasi ke WebAssembly untuk proses di browser
//! (`polify-wasm`) dan dipakai oleh API Axum untuk fitur server.

pub mod coords;
pub mod dbf;
pub mod export;
pub mod geo;
pub mod profiles;
pub mod shp;
pub mod utm;
pub mod validate;
pub mod zipcheck;

pub use geo::Coord;
pub use profiles::{PortalsConfig, ResolvedProfile};

/// Konfigurasi profil portal. Sumber kebenaran yang sama dimuat oleh frontend.
pub const PORTAL_PROFILES_JSON: &str = include_str!("../../../config/portal-profiles.json");

impl PortalsConfig {
    /// Muat konfigurasi bawaan dari `config/portal-profiles.json`.
    pub fn default_config() -> Self {
        Self::from_json(PORTAL_PROFILES_JSON).expect("config/portal-profiles.json harus valid")
    }
}
