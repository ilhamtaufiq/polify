# Uji pasar Polify (3 bulan)

Dokumen ini adalah kerangka. Angka ambang di dalamnya adalah usulan awal, bukan hasil riset, dan perlu disepakati sebelum uji dimulai.

## Tujuan

Menjawab tiga pertanyaan secara berurutan:

1. **Apakah orang mau menyelesaikan alur sampai ZIP diunduh?** (kebutuhan nyata)
2. **Apakah ZIP dari Polify diterima portal?** (nilai yang sebenarnya, bukan rasa percaya)
3. **Apakah konsultan mau membayar, dan berapa?** (model bisnis)

Tujuan ini tidak mengukur popularitas halaman.

## Prasyarat sebelum uji dimulai

- [ ] Aturan `config/portal-profiles.json` sudah diverifikasi ke juknis resmi, atau setidaknya ditandai "belum terverifikasi" di setiap aturan yang dipakai saat uji.
- [ ] Template AMDALnet resmi sudah dibandingkan dengan kolom DBF kita.
- [ ] Keputusan `.cpg` sudah dibuat (sumber sekunder menyebut "tepat 4 ekstensi").
- [ ] Penyedia tile peta sudah dipilih untuk trafik uji.
- [ ] Persetujuan pengguna uji tercatat, dan data yang dikumpulkan tidak mengandung data lahan klien.

## Segmen dan sampel

| Segmen | Target sampel | Sumber |
| --- | --- | --- |
| Konsultan dokumen lingkungan/KKPR di Cianjur dan sekitarnya | 10 wawancara, 3 pilot | jaringan langsung, bukan iklan berbayar |
| Pemrakarsa usaha non-UMK yang wajib melampirkan polygon di OSS RBA atau AMDALnet | 10 wawancara, 5 usability test | komunitas lokal, dinas terkait jika memungkinkan |

**Revisi segmen (dicatat):** segmen pelaku usaha kecil dihapus dari uji. Menurut sumber sekunder tentang PP 28/2025 Pasal 32, usaha mikro berisiko rendah cukup dengan pernyataan mandiri dan koordinat titik, sehingga mereka bukan pengguna yang tepat untuk menguji nilai SHP. Usaha kecil dan menengah belum jelas dari sumber yang ada. Semua ini harus diverifikasi ke naskah resmi PP 28/2025 (Pasal 12–34 dan Pasal 32) sebelum uji dimulai.

## Hipotesis dan metrik

| # | Hipotesis | Metrik | Ambang usulan untuk lanjut |
| --- | --- | --- | --- |
| H1 | Pengguna bisa membuat polygon tanpa bantuan | dari 5 usability test, berapa yang menyelesaikan langkah 1–4 tanpa bertanya | minimal 4 dari 5 |
| H2 | Pemeriksaan sebelum unggah menangkap masalah nyata | dari ZIP yang diuji, berapa yang mendapat catatan pemeriksaan | catatan muncul pada sebagian besar percobaan yang memang bermasalah |
| H3 | ZIP Polify diterima portal | dari pilot, berapa ZIP yang diterima tanpa revisi | dicatat apa adanya; tidak ada ambang sebelum data pertama ada |
| H4 | Konsultan mau memakai Polify untuk klien berikutnya | dari 3 pilot, berapa yang memakai lagi dalam 2 minggu | minimal 2 dari 3 |
| H5 | Konsultan bersedia membayar untuk fitur lanjutan | jawaban pada tiga harga uji (lihat di bawah) | ada tingkat harga yang diterima oleh mayoritas peserta |

**Catatan H3:** ini satu-satunya ukuran yang membuktikan nilai produk. Jika portal menolak ZIP Polify, produk belum layak dijual apa pun hasil H1 dan H4.

## Pengumpulan data

- **Wawancara:** catatan tertulis tanpa nama lengkap. Peserta diberi kode (K1, K2, ... untuk konsultan; U1, U2, ... untuk pelaku usaha).
- **Usability test:** rekam layar dengan izin. Catat langkah yang macet, bukan hanya hasil akhir.
- **Hasil unggah (H3):** peserta melaporkan sendiri apakah ZIP diterima atau ditolak, dan alasan penolakan jika ada. Salin pesan penolakan persis seperti yang ditampilkan portal.
- **Analitik web (opsional):** hanya hitungan halaman dan langkah, tanpa data pribadi atau koordinat lahan. Pilih alat yang tidak menyimpan data identitas.

Jangan pernah mengumpulkan koordinat lahan klien ke server untuk keperluan uji. Pemeriksaan tetap berjalan di browser.

## Uji harga (H5)

Gunakan tiga harga uji dan tanya setiap peserta: "Pada harga ini, Anda akan membeli?" Urutan harga diacak. Angka harga belum ditetapkan dan harus diputuskan oleh pemilik produk sebelum uji.

- Harga A: [isi]
- Harga B: [isi]
- Harga C: [isi]

Catat jawaban "ya", "tidak", dan "tidak tahu" untuk tiap harga. Jangan menjanjikan layanan apa pun yang belum tersedia.

## Jadwal

**Bulan 1: persiapan dan wawancara**
- Selesaikan prasyarat.
- Wawancara 10 konsultan dan 10 pelaku usaha.
- Usability test 5 pelaku usaha (H1).

**Bulan 2: pilot**
- Pilih 3 konsultan untuk memakai Polify pada klien nyata.
- Catat setiap ZIP yang diunduh dan hasil unggahnya (H2, H3).
- Cek ulang pemakaian setelah 2 minggu (H4).

**Bulan 3: harga dan keputusan**
- Jalankan uji harga (H5).
- Tulis ringkasan dan rekomendasi: lanjut, ubah, atau berhenti.

## Keputusan di akhir

| Hasil | Tindakan |
| --- | --- |
| H3 gagal berulang kali | hentikan promosi; perbaiki aturan dan layout DBF dulu |
| H3 berhasil, H4 gagal | ubah fokus ke alur yang paling dipakai, cari tahu penyebab berhenti |
| H3 dan H4 berhasil, H5 gagal | pertahankan layanan gratis, coba model harga lain |
| Semua berhasil | lanjut ke fitur batch dan akun (tahap 6 di README) |

## Etika

- Peserta tahu bahwa mereka sedang ikut uji, dan boleh berhenti kapan saja.
- Tidak ada testimoni yang dibuat atau diedit tanpa persetujuan tertulis.
- Hasil negatif dicatat dengan jujur. Uji ini untuk mengetahui kebenaran, bukan untuk membenarkan keputusan yang sudah ada.

## Keputusan model gratis (dicatat)

- **Paket dasar gratis untuk semua pengguna**, perorangan maupun usaha. Tanpa akun dan tanpa batas pemakaian.
- Alasan: batas pemakaian per orang tidak bisa ditegakkan tanpa auth, dan auth belum dibangun sebelum H3 terjawab.
- Monetisasi berdasarkan fitur yang membutuhkan akun (batch banyak lahan, penyimpanan proyek per klien, akun tim), bukan berdasarkan jenis pengguna.
- Paket konsultan tetap "Segera hadir" sampai uji harga (H5) selesai.
- Risiko yang diterima: konsultan bisa memakai versi gratis untuk banyak lahan secara manual. Jika ini sering terjadi, itu menjadi data untuk menentukan fitur berbayar.

## Yang belum diketahui

- Apakah portal benar-benar menerima ZIP dengan layout yang kita pakai.
- Berapa banyak konsultan yang aktif mengurus OSS RBA dan Amdalnet di Cianjur.
- Apakah pesaing sudah punya basis pengguna yang sulit disaingi.
