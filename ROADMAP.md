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
| **Format** | 7-Zip (`.7z`) | LZMA, LZMA2, BCJ2, PPMd | **Planned** | Enum tersedia di `types.rs` |
| **Format** | Zstandard (`.zst`, `.tar.zst`) | RFC 8878 Zstandard high-speed | **Planned** | Roadmap Fase 1 |
| **Format** | XZ (`.tar.xz`, `.xz`) | LZMA2 compression | **Planned** | Roadmap Fase 1 |
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
| **GUI & UX** | Drag-and-Drop (DND) Masuk & Keluar | Interaksi langsung dengan file manager | **Planned** | Roadmap Fase 2 |
| **GUI & UX** | Smart Extraction ("Extract Here - Smart") | Deteksi single-folder vs loose files | **Planned** | Roadmap Fase 2 |
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
