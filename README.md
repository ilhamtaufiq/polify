# Polify

> Buat file SHP polygon untuk OSS RBA & Amdalnet, gratis, langsung dari browser.

Polify membantu pengguna awam membuat Shapefile (SHP) polygon lokasi usaha atau kegiatan
tanpa ArcGIS atau QGIS. Satu file ZIP hasil ekspor disusun agar memenuhi gabungan aturan
paling ketat dari Amdalnet dan OSS RBA.

**Polify tidak berafiliasi dengan OSS RBA, Amdalnet, KLHK, atau ATR/BPN.**
Keputusan akhir ada pada sistem OSS RBA, Amdalnet, dan instansi terkait (PU/ATR-BPN).
Polify tidak menjamin file "pasti lolos validasi". Cocokkan hasil dengan panduan resmi
terbaru dan hasil KKPR.

## Status

| Tahap | Isi | Status |
| --- | --- | --- |
| 1 | `polify-core` + WASM: validasi polygon, konversi koordinat, SHP/DBF/ZIP, pemeriksaan ZIP | Selesai (uji 48 lulus) |
| 2 | Situs Astro: landing, SEO dasar | Belum |
| 3 | Impor (GeoJSON/KML/CSV/SHP ZIP), input koordinat, "Cek ZIP SHP saya" | Belum |
| 4 | API Axum (`/v1/*`), OpenAPI | Belum |
| 5 | Blog, konten SEO, laporan HTML/PDF | Belum |
| 6 | Akun, proyek tersimpan, batch (fase 2) | Belum |

## Struktur monorepo

```
config/portal-profiles.json   Aturan portal (dimuat Rust dan frontend; bukan hard-code)
crates/polify-core/           Logika geospasial: validate, coords, utm, shp, dbf, zipcheck, export
crates/polify-wasm/           Pembungkus wasm-bindgen untuk editor di browser
apps/web/                     (tahap 2) Astro + island editor
apps/api/                     (tahap 4) Axum + Tokio
```

## Menjalankan dan menguji (tahap 1)

Prasyarat: Rust stable (1.75+) dan target `wasm32-unknown-unknown`.

```bash
# Uji seluruh crate inti (unit + integrasi)
cargo test -p polify-core

# Build WASM
rustup target add wasm32-unknown-unknown
cargo build -p polify-wasm --target wasm32-unknown-unknown --release

# Bungkus untuk browser (memerlukan wasm-pack)
cd crates/polify-wasm && wasm-pack build --target web --release
```

Antarmuka WASM memakai JSON string agar kontraknya mudah diuji:

| Fungsi | Masukan | Keluaran |
| --- | --- | --- |
| `portal_config()` | - | JSON konfigurasi profil |
| `validate_ring(ring)` | `[[lon,lat],...]` | `PolygonReport` JSON |
| `autofix_ring(ring)` | `[[lon,lat],...]` | `{ring, applied}` JSON |
| `convert_coordinates(input)` | `{"mode":"dd\|dms\|utm\|pair", ...}` | DD, DMS, dan UTM sekaligus |
| `export_zip(request, profileId)` | `{basename, ring, atribut}` | byte ZIP (`Uint8Array`) |
| `check_zip(bytes, profileId, expected?)` | byte ZIP | `CheckReport` JSON |
| `zip_badges(bytes, expected?)` | byte ZIP | badge lulus/gagal per profil |

Kesalahan dilempar sebagai string JSON `{"code": "...", "message": "..."}`.

## Aturan inti

- **Kompatibilitas ganda.** Profil `combined` (strictest-common) mewajibkan
  `.shp .dbf .shx .prj .cpg` di root ZIP dengan nama dasar yang sama.
- **Validasi geometri.** Minimal 3 titik unik, ring tertutup, tanpa bow-tie, duri/sliver
  diberi peringatan, arah ring searah jarum jam (standar ESRI), koordinat di wilayah
  Indonesia, serta deteksi lintang/bujur tertukar. Tumpang tindih antar-polygon juga diperiksa.
- **Perbaikan otomatis** hanya untuk hal yang tidak mengubah bentuk: menutup ring, menukar
  sumbu, menghapus duplikat dan duri, serta membalik arah. Bow-tie tidak diperbaiki otomatis.
- **Pemeriksaan ulang.** Ekspor membaca kembali ZIP yang dihasilkan, lalu membandingkan
  jumlah fitur dan luas dengan data peta. Berkas tidak diberikan bila pemeriksaan gagal.
- **Proses di browser.** Pada mode gratis, data diproses di browser melalui WASM dan tidak
  dikirim ke server.

## Asumsi yang harus diverifikasi (TODO verifikasi)

Nilai berikut bersumber dari praktik umum, bukan dari panduan resmi yang sudah diverifikasi.
Lihat `config/portal-profiles.json`.

- `flatStructure` (file di root ZIP) untuk Amdalnet dan OSS RBA.
- Daftar `requiredExtensions` dan CRS/encoding untuk OSS RBA.
- `maxFileSizeMB` dan `maxFeatures` untuk kedua portal. Nilai `null` berarti belum diketahui
  dan tidak diberlakukan. Polify tidak mengarang batas ini.
- Satu fitur polygon per file, serta nama field DBF maksimal 10 karakter (batas dBASE).
- Ambang heuristik internal (`minAreaM2`, `areaWarnHa`) bukan aturan resmi.
- Juknis Amdalnet: lima ekstensi wajib dan UTF-8 serta WGS84 mengikuti juknis penyiapan peta.

## Checklist SEO (tahap 2)

Belum dikerjakan. Akan mencakup meta per halaman, JSON-LD (Organization, WebSite,
SoftwareApplication, FAQPage, HowTo, BreadcrumbList), sitemap, robots, dan Core Web Vitals.
