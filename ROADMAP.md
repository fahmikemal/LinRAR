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
| **Format** | Container & Disk Images (ISO 9660, Joliet) | Pure Rust container browsing, extraction & conversion | **Full** | `iso_engine.rs` |
| **Keamanan** | Enkripsi AES-256 ZIP murni Rust | WinZip AES-256 standard | **Full** | `zip_engine.rs` (`AesMode::Aes256`) |
| **Keamanan** | Enkripsi Header / Encrypt Filenames | RAR `-hp`, 7z header encryption | **Full** | `rar_engine.rs` (`-hp`) |
| **Keamanan** | Virus & Threat Scanner | ClamAV + Heuristic signatures | **Full** | `scanner.rs` (`ArchiveScanner`) |
| **Keamanan** | Path Traversal / Zip Slip Defense | Strict path sanitization (`enclosed_name`) | **Full** | `zip_engine.rs`, `tar_engine.rs` |
| **Keamanan** | Secure File Shredding (DoD / Gutmann) | Overwrite before unlink | **Full** | `shredder.rs` (`FileShredder`) |
| **Integritas** | Uji Integritas (Test Archive) | CRC32, MD5, SHA-256 checksums | **Full** | `test_archive()` di semua engine |
| **Integritas** | Archive Repair (Perbaikan Arsip) | Central directory & header recovery | **Full** | `repair.rs` (`ArchiveRepair`) |
| **Integritas** | Recovery Record (Reed-Solomon) | 1%–1000% parity data blocks | **Full** | `rar_engine.rs` + Protect dialog |
| **Integritas** | Archive Lock (Proteksi Modifikasi) | Anti-tamper lock flag enforcement | **Full** | `rar_engine.rs` + lock guard di GUI |
| **Arsip** | Multi-Volume Spanning (Split Archives) | Partisi `.001` / `.part01` & chunk builder | **Full** | `volume.rs` (`VolumeManager`) |
| **Arsip** | Archive Converter | Konversi otomatis antar-format | **Full** | `converter.rs` (`ArchiveConverter`) |
| **Arsip** | Archive Content Report | Ekspor inventaris HTML & TXT | **Full** | `report.rs` (`ArchiveReport`) |
| **Arsip** | Smart Bypass (High-Speed Archiving) | Skip re-compressing media (mp4, jpg) | **Full** | `bypass.rs` (`SmartBypass`) + ZIP/7z engines |
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
    - Perintah CLI lengkap (`linrar a`, `linrar l`, `linrar x`, `linrar t`, `linrar convert`).

### K. Smart Bypass / High-Speed Archiving (Deteksi Berkas Non-Kompresibel & Heuristik Entropi)
- **Penyempurnaan**:
  - Engine `SmartBypass` di `src/core/bypass.rs` yang secara otomatis mendeteksi berkas yang sudah terkompresi untuk mengabaikan (*bypass*) proses kompresi ulang yang membuang siklus CPU dan memperlambat pembuatan arsip (fitur benchmark ala WinRAR "Store uncompressed" dan Bandizip "High Speed Archiving").
  - **Registry Ekstensi Komprehensif**: Mendukung 40+ ekstensi non-kompresibel mencakup video (`mp4`, `mkv`, `avi`, `mov`), audio (`mp3`, `aac`, `ogg`, `flac`, `opus`), gambar modern (`jpg`, `png`, `webp`, `gif`, `avif`, `heic`), paket distribusi (`apk`, `jar`, `deb`, `rpm`), dokumen kontainer (`docx`, `xlsx`, `pdf`, `epub`), dan arsip (`zip`, `7z`, `rar`, `zst`, `gz`).
  - **Heuristik Shannon Entropy Terpadu**: Untuk berkas tanpa ekstensi standar atau berekstensi asing berukuran $\ge 4$ KB, engine menghitung Shannon entropy dari sampel data awal. Jika entropi $\ge 7.6$ bit/byte (mengindikasikan data terenkripsi atau stream kompresi acak), berkas secara otomatis diklasifikasikan sebagai non-kompresibel.
  - **Dukungan ZIP & 7-Zip Multi-Method**:
    - Pada arsip ZIP, setiap berkas non-kompresibel otomatis disimpan menggunakan `CompressionMethod::Stored` sementara berkas teks/kode tetap dikompresi dengan `CompressionMethod::Deflated`.
    - Pada arsip 7-Zip non-solid, berkas bypass disimpan menggunakan method `COPY` (Store), menghemat pemrosesan LZMA2 secara masif.
  - **Dukungan Penuh GUI & CLI**:
    - Dialog GUI *Add to Archive*: Checkbox interaktif "⚡ Smart Bypass (High-Speed Archiving)" dengan hover tooltip informatif.
    - Opsi CLI: Diaktifkan secara default pada `linrar a`, dan dapat dimatikan jika diinginkan melalui flag `--no-smart-bypass`.

### L. Secure File Shredding / Sanitasi Berkas Permanen (DoD 5220.22-M & Multi-Pass)
- **Penyempurnaan**:
  - Engine sanitasi data aman `FileShredder` di `src/core/shredder.rs` untuk menghapus berkas rahasia secara permanen agar tidak dapat dipulihkan (*unrecoverable*) oleh software forensik / undelete (fitur benchmark ala PeaZip "Secure delete" dan WinRAR "Wipe files after archiving").
  - **Standar Sanitasi Multi-Pass**:
    - *Quick Zero (1-pass)*: Penulisan byte `0x00` tunggal secara cepat dan optimal untuk media NVMe/SSD dengan kontrol TRIM.
    - *Pseudorandom (1-pass)*: Penulisan deret acak berkecepatan tinggi via generator XorShift128+ terenkripsi dari `/dev/urandom`.
    - *DoD 5220.22-M (3-pass)*: Standar Departemen Pertahanan AS (Pass 1: pola biner `0x55`, Pass 2: pola komplemen `0xAA`, Pass 3: deret acak).
    - *Gutmann Lite (7-pass)*: Kombinasi rotasi bit dan multi-pattern acak untuk media magnetik/legacy.
  - **Protokol Keamanan Tingkat Rendah**:
    - Memaksa hak akses berkas (*file permission*) menjadi *writable* (`0o600`) sebelum proses penulisan ulang.
    - Pemanggilan sinkronisasi perangkat keras `file.sync_all()` setelah setiap *pass* penulisan untuk menguras cache OS ke blok penyimpanan fisik.
    - Pemotongan ukuran berkas (*truncate*) ke 0 byte setelah selesai dioverwrite.
    - Pengacakan nama berkas (*obfuscate filename*) di tingkat direktori sebelum berkas di-*unlink* dari sistem file guna membersihkan jejak nama berkas asli dari jurnal inode/direktori.
    - Dukungan sanitasi rekursif untuk struktur pohon direktori (*bottom-up traversal*).
  - **Integrasi Penuh GUI & CLI**:
    - Dialog GUI *Add to Archive*: Opsi "🗑 Delete files after archiving", sub-opsi "🔒 Secure Shred (Overwrite data before unlink)", dan ComboBox pemilihan algoritma standar penghapusan.
    - Opsi CLI saat mengarsipkan: `linrar a <arch> <files> --delete`, `linrar a <arch> <files> --shred`, dan `--shred-method <quick|random|dod|gutmann>`.
    - Perintah CLI mandiri: `linrar shred <files/dirs>... [--method <quick|random|dod|gutmann>]`.

### M. Container & Disk Image Reader (ISO 9660 & Joliet Extension)
- **Implementasi**:
  - Engine murni Rust `IsoEngine` di `src/core/iso_engine.rs` tanpa ketergantungan pada utilitas eksternal (`p7zip`, `isoinfo`, `mount`) untuk membuka, menjelajah, memverifikasi, dan mengekstrak berkas citra disk (`.iso`).
  - **Spesifikasi ISO 9660 & Joliet (ECMA-119)**:
    - *Volume Descriptors*: Membaca Primary Volume Descriptor (PVD) pada sektor 16 dan Supplementary Volume Descriptor (SVD) dengan ekstensi Joliet UCS-2 / UTF-16BE.
    - *Hierarchical Traversal*: Mengurai record direktori berlapis secara rekursif dengan penanganan padding batas sektor 2048-byte dan pencegahan siklus rekursif (*infinite loop defense* via `HashSet`).
    - *Decoding Nama Berkas*: Mendukung karakter Unicode penuh via Joliet UCS-2 serta pembersihan suffix versi ISO 9660 (`;1`) dan titik trailing secara otomatis.
    - *Metadata & Timestamps*: Membaca tanggal dan waktu perekaman 7-byte standar ISO 9660 (`YYYY-MM-DD HH:MM:SS`), atribut izin file POSIX, label volume, dan informasi sistem di dialog Info Arsip.
    - *Keamanan Path Traversal*: Perlindungan terintegrasi terhadap serangan directory traversal (`..`).
  - **Integrasi Penuh LinRAR**:
    - Deteksi otomatis tipe berkas via ekstensi `.iso` dan signature magic byte `CD001` pada sektor 16 (offset 32768).
    - Ekstraksi Pintar (*Smart Extraction*): Deteksi folder tunggal vs file berserakan pada berkas ISO.
    - Konverter Arsip (*Format Converter*): Konversi langsung berkas `.iso` menjadi `.zip`, `.tar.gz`, `.7z`, atau format arsip lainnya.
    - Integrasi CLI: Mendukung `linrar l <file.iso>`, `linrar x <file.iso>`, `linrar t <file.iso>`, dan `linrar convert <file.iso> <target_fmt>`.
    - Integrasi GUI: Penjelajahan direktori virtual, dialog info arsip, verifikasi integritas citra, Quick Look preview berkas, dan dialog ekstraksi.




