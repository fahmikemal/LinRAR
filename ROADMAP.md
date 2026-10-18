# LinRAR Feature Matrix & Industry Benchmark Roadmap (October 2026)

Dokumen ini melacak perbandingan fitur **LinRAR** terhadap standar *archive manager* mature dunia (WinRAR 7.x, 7-Zip 24–26.x, PeaZip 10–11.x, Bandizip, NanaZip, Ark/KDE, File Roller) per Oktober 2026.

---

## 1. Status Audit Fitur LinRAR

| Kategori | Fitur | Standar Industri Global | Status LinRAR | Keterangan Codebase |
| :--- | :--- | :--- | :---: | :--- |
| **Format** | ZIP (Read/Write) | Deflate, Bzip2, Zstd, LZMA | **Full** | `zip_engine.rs` |
| **Format** | TAR, TAR.GZ, TAR.BZ2 | POSIX tarball, Gzip, Bzip2 | **Full** | `tar_engine.rs` |
| **Format** | RAR (RAR4 & RAR5 Read) | Pure parsing & UnRAR CLI | **Full** | `rar_parser.rs` + `rar_engine.rs` |
| **Format** | RAR (Extraction/Creation) | Proprietary RARLAB unrar/rar | **Full** | CLI wrapper `rar_engine.rs` |
| **Format** | 7-Zip (`.7z`) | LZMA, LZMA2, BCJ2, PPMd | **Full** | `sevenzip_engine.rs` + `sevenz-rust2` |
| **Format** | Zstandard (`.zst`, `.tar.zst`, `.tzst`) | RFC 8878 Zstandard high-speed | **Full** | `tar_engine.rs` + `zstd` |
| **Format** | XZ (`.tar.xz`, `.txz`, `.xz`) | LZMA2 compression | **Full** | `tar_engine.rs` + `xz2` |
| **Format** | Container Images (ISO, DMG, WIM) | Read-only image extraction | **Planned** | Roadmap Fase 1 |
| **Keamanan** | Enkripsi AES-256 ZIP murni Rust | WinZip AES-256 standard | **Full** | `zip_engine.rs` (`AesMode::Aes256`) |
| **Keamanan** | Enkripsi Header / Encrypt Filenames | RAR `-hp`, 7z header encryption | **Full** | `rar_engine.rs` (`-hp`) |
| **Keamanan** | Virus & Threat Scanner | ClamAV + Heuristic signatures | **Full** | `scanner.rs` (`ArchiveScanner`) |
| **Keamanan** | Path Traversal / Zip Slip Defense | Strict path sanitization (`enclosed_name`) | **Full** | `zip_engine.rs`, `tar_engine.rs` |
| **Keamanan** | Secure File Shredding (DoD / Gutmann) | Overwrite before unlink | **Planned** | Roadmap Fase 3 |
| **Integritas** | Uji Integritas (Test Archive) | CRC32, MD5, SHA-256 checksums | **Full** | `test_archive()` di semua engine |
| **Integritas** | Archive Repair (Perbaikan Arsip) | Central directory & header recovery | **Full** | `repair.rs` (`ArchiveRepair`) |
| **Integritas** | Recovery Record (Reed-Solomon) | 1%–1000% parity data blocks | **Full** | `rar_engine.rs` + Protect dialog |
| **Integritas** | Archive Lock (Proteksi Modifikasi) | Anti-tamper lock flag enforcement | **Full** | `rar_engine.rs` + lock guard di GUI |
| **Arsip** | Multi-Volume Spanning (Split Archives) | Partisi `.001` / `.part01` & chunk builder | **Full** | `volume.rs` (`VolumeManager`) |
| **Arsip** | Archive Converter | Konversi otomatis antar-format | **Full** | `converter.rs` (`ArchiveConverter`) |
| **Arsip** | Archive Content Report | Ekspor inventaris HTML & TXT | **Full** | `report.rs` (`ArchiveReport`) |
| **Arsip** | Smart Bypass (High-Speed Archiving) | Skip re-compressing media (mp4, jpg) | **Planned** | Roadmap Fase 2 |
| **Arsip** | Self-Extracting Archive (SFX) | Linux ELF executable stub | **Planned** | Roadmap Fase 3 |
| **GUI & UX** | Unified CSD Titlebar | Vector-drawn controls, borderless resize | **Full** | `app.rs` (`custom_title_bar`) |
| **GUI & UX** | Quick Look / In-Archive File Preview | Preview teks/gambar langsung via default system app | **Full** | `app.rs` (`trigger_view_file`, double-click) |
| **GUI & UX** | Drag-and-Drop (DND) File Manager | Drag file/arsip dari Dolphin/Nautilus dengan visual drop-overlay & auto-open/add | **Full** | `app.rs` (`handle_dropped_files`, `render_dnd_overlay`) |
| **GUI & UX** | Smart Extraction ("Extract Here - Smart") | Deteksi single-folder vs loose files | **Full** | `smart_extract.rs` (`SmartExtractor`) |
| **GUI & UX** | Code Page / Encoding Selector | Anti-mojibake (Shift-JIS, CP949, GBK) | **Planned** | Roadmap Fase 2 |
| **Sistem** | Unix Metadata (Permissions, mtime) | Mode octal `-rwxr-xr-x`, timestamp | **Full** | `zip_engine.rs`, `tar_engine.rs` |
| **Sistem** | Hardware Benchmark Multithreaded | Rayon hashing & integer MIPS test | **Full** | `benchmark.rs` (`BenchmarkRunner`) |
| **Sistem** | Desktop Integration | `.desktop`, icons, MIME association | **Full** | `linrar.desktop`, `assets/icons/` |

---

## 2. Fitur-Fitur Parsial yang Disempurnakan (October 2026 Sprint)

Berikut adalah detail penyempurnaan fitur yang sebelumnya setengah terimplementasi:

### A. Enkripsi AES-256 Murni Rust untuk Format ZIP
- **Sebelumnya**: Opsi kata sandi hanya diteruskan ke CLI RAR, sedangkan pembuatan ZIP murni mengabaikan password.
- **Penyempurnaan**:
  - Mengintegrasikan `with_aes_encryption(zip::AesMode::Aes256, password)` di `zip_engine.rs`.
  - Ekstraksi ZIP terenkripsi menggunakan `by_index_decrypt(i, password.as_bytes())`.
  - Mendeteksi status `encrypted` pada setiap entri file ZIP di tampilan tabel GUI.

### B. Preservasi Metadata & Hak Akses Unix (Permissions & Timestamps)
- **Sebelumnya**: Kolom `modified` dan `attributes` pada ZIP & TAR menampilkan string default `"-"`.
- **Penyempurnaan**:
  - Mengonversi `unix_mode()` dan header mode TAR menjadi format atribut POSIX standar yang ramah pengguna (contoh: `-rwxr-xr-x`, `drwxr-xr-x`).
  - Mengonversi timestamp `mtime` dan `last_modified()` menjadi representasi tanggal waktu terformat (`YYYY-MM-DD HH:MM:SS`).
  - Menyimpan permission file lokal asli saat membuat arsip ZIP dan merestorasi permission file (`chmod`) saat mengekstrak di Linux.

### C. Multi-Volume Spanning Engine (`src/core/volume.rs`)
- **Sebelumnya**: Hanya flag parameter `-v` pada RAR CLI, tidak bisa memecah arsip ZIP/TAR atau file besar lainnya.
- **Penyempurnaan**:
  - Engine `VolumeManager` yang mampu memecah (*split*) file arsip apa pun menjadi beberapa bagian volume sesuai ukuran yang diinginkan pengguna (contoh: `10M`, `100M`, `700M`, `1G`, `4480M`).
  - Engine penggabungan kembali (*join*) yang mendeteksi pola `.001`, `.002` atau `.part01.rar` dan merekonstruksi arsip utuh dengan verifikasi checksum.

### D. Penegakan Archive Locking (Anti-Modifikasi)
- **Sebelumnya**: Hanya menjalankan `rar k` tanpa penegakan proteksi di sisi GUI LinRAR.
- **Penyempurnaan**:
  - Melakukan validasi proteksi sebelum operasi penambahan file, penggantian nama (*rename*), atau penghapusan file (*delete*). Jika status arsip `is_locked`, operasi langsung dibatalkan dengan peringatan visual bahwa arsip terkunci.

### E. Solid Archiving Semantics
- **Sebelumnya**: TAR.GZ dan TAR.BZ2 ditandai `is_solid: false` padahal secara arsitektural merupakan solid stream.
- **Penyempurnaan**:
  - Menandai TAR.GZ dan TAR.BZ2 sebagai `is_solid: true` secara akurat pada `ArchiveInfo`.

### F. Quick Look & In-Archive File Preview (Double-Click to View)
- **Sebelumnya**: Tombol "View" saat melihat file di dalam arsip hanya menampilkan pesan petunjuk untuk mengekstrak manual.
- **Penyempurnaan**:
  - Mengekstrak file terpilih secara otomatis ke direktori pratinjau sementara (`/tmp/linrar_preview_<pid>/...`) dan membukanya langsung menggunakan aplikasi default sistem via `xdg-open`.
  - Mendukung pembukaan file melalui klik dua kali (*double-click*) pada tabel file di dalam arsip maupun pada sistem file lokal.

### G. Integrasi Drag-and-Drop (DND) Desktop Linux
- **Penyempurnaan**:
  - Dukungan drag-and-drop langsung dari file manager desktop (Nautilus GNOME, Dolphin KDE, Thunar XFCE).
  - Overlay visual transparan elegan dengan aksen tema (Dark & Light) saat file melayang di atas jendela LinRAR.
  - Membuka file arsip secara otomatis jika satu file arsip didrop ke LinRAR.
  - Menampilkan dialog "Add to Archive" dengan file yang didrop jika dilepas ke arsip yang sedang terbuka atau browser lokal.
  - Melindungi arsip yang terkunci (`is_locked`) agar tidak dapat disusupi file drop tanpa izin.

### H. Smart Extraction ("Ekstraksi Pintar" ala Bandizip / PeaZip)
- **Penyempurnaan**:
  - Engine `SmartExtractor` di `src/core/smart_extract.rs` yang secara cerdas mendeteksi hierarki dan struktur isi arsip sebelum diekstrak.
  - **Mencegah Duplikasi Folder**: Jika seluruh isi arsip sudah berada di dalam satu direktori *root*, arsip diekstrak langsung ke folder tujuan tanpa membungkusnya ke subdirektori tambahan (`target/folder/...` bukan `target/nama_arsip/folder/...`).
  - **Mencegah Archive-Bomb**: Jika arsip berisi berkas-berkas lepas (*loose files*) di root, arsip secara otomatis diekstrak ke dalam subdirektori baru dengan nama arsip (`target/nama_arsip/...`), menjaga kerapian folder tujuan atau Desktop pengguna.
  - **Pembersihan Nama Ekstensi Majemuk**: Penamaan subfolder cerdas membersihkan ekstensi majemuk seperti `.tar.gz`, `.tar.bz2`, `.tar.xz`, `.part01.rar`, `.zip.001`.
  - **Dukungan Penuh GUI & CLI**: Checkbox "Ekstraksi Pintar" diaktifkan secara default pada dialog ekstraksi GUI, serta flag `--smart` pada perintah CLI (`linrar x --smart <archive> [dest]`).

### I. Format Modern Linux: Zstandard (.tar.zst) & XZ (.tar.xz)
- **Penyempurnaan**:
  - Mengintegrasikan engine dekompresi dan kompresi streaming murni Rust untuk standar RFC 8878 Zstandard (`zstd`) dan LZMA2 (`xz2`).
  - Mendukung pembuatan arsip (*create*), pembacaan inventaris entri (*list*), uji integritas (*test archive*), pratinjau instan (*quick look*), ekstraksi cerdas (*smart extraction*), dan konversi (*converter*) untuk berkas `.tar.zst`, `.tzst`, `.tar.xz`, dan `.txz`.
  - Desain `TarWriter` streaming dengan pemanggilan `.finish()` eksplisit pada encoder untuk menjamin penulisan footer, indeks checksum, dan integritas data format XZ dan Zstandard.
  - Integrasi opsi format `.tar.xz` dan `.tar.zst` pada dialog GUI "Add to Archive" dan "Convert Archive", serta flag format pada perintah CLI (`linrar a`, `linrar x`, `linrar l`, `linrar t`, `linrar convert`).

### J. Format 7-Zip (.7z) Murni Rust Berstandar Industri
- **Penyempurnaan**:
  - Mengembangkan engine murni Rust `SevenZipEngine` di `src/core/sevenzip_engine.rs` berbasis `sevenz-rust2` tanpa ketergantungan binary eksternal `p7zip`.
  - **Algoritma Kompresi LZMA2**: Mendukung tingkatan kompresi dari *Store*, *Fast*, *Normal*, hingga *Ultra/Best* dengan konfigurasi solid dan non-solid block streaming.
  - **Enkripsi AES-256 & Header Encryption**: Mendukung enkripsi isi file serta opsi enkripsi daftar nama file (*encrypt header / filenames*) yang menyembunyikan struktur isi arsip hingga password yang benar dimasukkan.
  - **Preservasi Struktur Folder & Metadata**: Penanganan direktori bertingkat secara rekursif menggunakan `ArchiveEntry::from_path` dan `walkdir`, menjaga hierarki folder dan atribut waktu (mtime/ctime).
  - **Multi-Volume Spanning**: Mendukung pemecahan arsip `.7z` menjadi berkas chunk berukuran tertentu (`.7z.001`, `.7z.002`, dst.) melalui integrasi `VolumeManager`.
  - **Integrasi Penuh GUI & CLI**:
    - Dialog GUI *Add to Archive*: Radio button format `.7z` dengan opsi enkripsi header dan split volume.
    - Dialog GUI *Extract*: Ekstraksi 7z dengan kata sandi dan integrasi *Smart Extraction*.
    - Dialog GUI *Convert Archive*: Konversi dua arah dari/ke format 7z (ZIP ↔ 7z, TAR ↔ 7z, RAR ↔ 7z).
    - Quick Look / Double-click preview: Pratinjau langsung file di dalam arsip `.7z`.
    - Uji integritas arsip (*test archive*) dengan verifikasi checksum CRC32 dan stream dekompresi.
    - Perintah CLI lengkap (`linrar a`, `linrar l`, `linrar x`, `linrar t`, `linrar convert`).

