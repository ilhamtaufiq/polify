# TODO Polify

Terakhir diperbarui: 2026-10-13

## Yang perlu disediakan (menunggu Anda)

- [ ] **Naskah resmi PP 28/2025** (PDF) untuk memverifikasi Pasal 12–34 dan Pasal 32. Taruh di `Downloads`, lalu kirim path-nya.
- [ ] **Template SHP AMDALnet resmi** dari dasbor AMDALnet, beserta `.dbf` dan `.prj`-nya.
- [ ] **Juknis unggah OSS RBA resmi.** Bisa juga berupa jawaban dari kontak@oss.go.id untuk pertanyaan: CRS yang diterima, komponen ZIP wajib, apakah `.cpg` diperlukan, dan batas ukuran file.
- [ ] **Keputusan harga paket konsultan.** Kolom harga masih kosong di `docs/uji-pasar.md`.
- [ ] **Keputusan penyedia tile peta untuk produksi.** Saat ini masih OpenStreetMap, yang tidak cocok untuk trafik publik.
- [ ] **Keputusan hosting** untuk `polify.cianjur.space`.
- [ ] **Keputusan `.cpg`.** Apakah tetap wajib di profil `combined`, sambil menunggu juknis.

## Pekerjaan saya (setelah input di atas tersedia)

- [ ] Verifikasi `config/portal-profiles.json` ke juknis resmi, dan isi sumber serta tanggalnya.
- [ ] Sesuaikan layout DBF dengan template AMDALnet.
- [ ] Uji ulang semua halaman di browser, desktop dan HP.
- [ ] Tambahkan tes Playwright untuk alur editor dan impor.
- [ ] Tampilkan luas poligon di editor (sesuai pemeriksaan OSS yang menghitung ulang luas).
- [ ] Buat `og.png` dan cek Core Web Vitals.
- [ ] Pastikan workflow CI berjalan di GitHub.
- [ ] Hapus `crates/polify-wasm/demo/`, karena sudah digantikan oleh `/impor` dan `/editor`.

## Uji pasar

Prasyarat dan jadwal ada di `docs/uji-pasar.md`. Uji hanya dimulai setelah aturan portal terverifikasi.
