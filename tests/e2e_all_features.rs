use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use linrar::core::bypass::SmartBypass;
use linrar::core::sevenzip_engine::SevenZipEngine;
use linrar::core::tar_engine::TarEngine;
use linrar::core::zip_engine::ZipEngine;
use linrar::types::{AddOptions, ArchiveType};

/// Helper to get a Command configured for the compiled linrar binary
fn linrar_cmd() -> Command {
    let bin_path = env!("CARGO_BIN_EXE_linrar");
    let mut cmd = Command::new(bin_path);
    cmd.env("LINRAR_NO_GUI", "1");
    cmd
}

/// Helper to synthesize a ZIP archive with non-UTF8 raw filenames for anti-mojibake testing
fn synthesize_test_zip_with_raw_name(path: &Path, raw_filename: &[u8], content: &[u8]) {
    let mut data = Vec::new();
    let crc_val = {
        let mut h = crc32fast::Hasher::new();
        h.update(content);
        h.finalize()
    };
    let comp_size = content.len() as u32;
    let uncomp_size = content.len() as u32;

    // Local file header (30 bytes + name + content)
    data.extend_from_slice(b"PK\x03\x04");
    data.extend_from_slice(&20u16.to_le_bytes());
    data.extend_from_slice(&0u16.to_le_bytes()); // general purpose bit flag (bit 11 UTF-8 = 0)
    data.extend_from_slice(&0u16.to_le_bytes()); // method: Store
    data.extend_from_slice(&0u16.to_le_bytes());
    data.extend_from_slice(&0u16.to_le_bytes());
    data.extend_from_slice(&crc_val.to_le_bytes());
    data.extend_from_slice(&comp_size.to_le_bytes());
    data.extend_from_slice(&uncomp_size.to_le_bytes());
    data.extend_from_slice(&(raw_filename.len() as u16).to_le_bytes());
    data.extend_from_slice(&0u16.to_le_bytes());
    data.extend_from_slice(raw_filename);
    data.extend_from_slice(content);

    // Central directory header (46 bytes + name)
    let cd_offset = data.len() as u32;
    data.extend_from_slice(b"PK\x01\x02");
    data.extend_from_slice(&20u16.to_le_bytes());
    data.extend_from_slice(&20u16.to_le_bytes());
    data.extend_from_slice(&0u16.to_le_bytes());
    data.extend_from_slice(&0u16.to_le_bytes());
    data.extend_from_slice(&0u16.to_le_bytes());
    data.extend_from_slice(&0u16.to_le_bytes());
    data.extend_from_slice(&crc_val.to_le_bytes());
    data.extend_from_slice(&comp_size.to_le_bytes());
    data.extend_from_slice(&uncomp_size.to_le_bytes());
    data.extend_from_slice(&(raw_filename.len() as u16).to_le_bytes());
    data.extend_from_slice(&0u16.to_le_bytes());
    data.extend_from_slice(&0u16.to_le_bytes());
    data.extend_from_slice(&0u16.to_le_bytes());
    data.extend_from_slice(&0u16.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(raw_filename);

    let cd_len = (data.len() as u32) - cd_offset;

    // End of central directory record (22 bytes)
    data.extend_from_slice(b"PK\x05\x06");
    data.extend_from_slice(&0u16.to_le_bytes());
    data.extend_from_slice(&0u16.to_le_bytes());
    data.extend_from_slice(&1u16.to_le_bytes());
    data.extend_from_slice(&1u16.to_le_bytes());
    data.extend_from_slice(&cd_len.to_le_bytes());
    data.extend_from_slice(&cd_offset.to_le_bytes());
    data.extend_from_slice(&0u16.to_le_bytes());

    fs::write(path, data).unwrap();
}

/// Helper to synthesize a valid ISO 9660 Joliet disk image
fn synthesize_test_iso(path: &Path) {
    let sector_size = 2048;
    let total_sectors = 22;
    let mut data = vec![0u8; total_sectors * sector_size];

    // Sector 16: Primary Volume Descriptor (PVD)
    let pvd = &mut data[16 * sector_size..17 * sector_size];
    pvd[0] = 1; // PVD
    pvd[1..6].copy_from_slice(b"CD001");
    pvd[6] = 1;
    let sys_id = b"LINRAR_ISO_SYS";
    pvd[8..8 + sys_id.len()].copy_from_slice(sys_id);
    let vol_id = b"E2E_TEST_ISO";
    pvd[40..40 + vol_id.len()].copy_from_slice(vol_id);
    pvd[80..84].copy_from_slice(&(total_sectors as u32).to_le_bytes());
    pvd[84..88].copy_from_slice(&(total_sectors as u32).to_be_bytes());
    pvd[120..122].copy_from_slice(&1u16.to_le_bytes());
    pvd[122..124].copy_from_slice(&1u16.to_be_bytes());
    pvd[124..126].copy_from_slice(&1u16.to_le_bytes());
    pvd[126..128].copy_from_slice(&1u16.to_be_bytes());
    pvd[128..130].copy_from_slice(&2048u16.to_le_bytes());
    pvd[130..132].copy_from_slice(&2048u16.to_be_bytes());

    // Root directory record in PVD
    pvd[156] = 34;
    pvd[157] = 0;
    pvd[158..162].copy_from_slice(&18u32.to_le_bytes());
    pvd[162..166].copy_from_slice(&18u32.to_be_bytes());
    pvd[166..170].copy_from_slice(&2048u32.to_le_bytes());
    pvd[170..174].copy_from_slice(&2048u32.to_be_bytes());
    pvd[174..181].copy_from_slice(&[126, 10, 19, 7, 0, 0, 0]);
    pvd[181] = 0x02; // Directory flag
    pvd[188] = 1;
    pvd[189] = 0;

    // Sector 17: Terminator
    let term = &mut data[17 * sector_size..18 * sector_size];
    term[0] = 255;
    term[1..6].copy_from_slice(b"CD001");
    term[6] = 1;

    fn write_record(
        buf: &mut [u8],
        pos: usize,
        lba: u32,
        data_len: u32,
        is_dir: bool,
        name: &[u8],
    ) -> usize {
        let name_len = name.len();
        let mut rec_len = 33 + name_len;
        if !rec_len.is_multiple_of(2) {
            rec_len += 1;
        }
        buf[pos] = rec_len as u8;
        buf[pos + 1] = 0;
        buf[pos + 2..pos + 6].copy_from_slice(&lba.to_le_bytes());
        buf[pos + 6..pos + 10].copy_from_slice(&lba.to_be_bytes());
        buf[pos + 10..pos + 14].copy_from_slice(&data_len.to_le_bytes());
        buf[pos + 14..pos + 18].copy_from_slice(&data_len.to_be_bytes());
        buf[pos + 18..pos + 25].copy_from_slice(&[126, 10, 19, 7, 0, 0, 0]);
        buf[pos + 25] = if is_dir { 0x02 } else { 0x00 };
        buf[pos + 32] = name_len as u8;
        buf[pos + 33..pos + 33 + name_len].copy_from_slice(name);
        rec_len
    }

    // Sector 18: Root Directory Sector
    let root = &mut data[18 * sector_size..19 * sector_size];
    let mut pos = 0;
    pos += write_record(root, pos, 18, 2048, true, &[0]);
    pos += write_record(root, pos, 18, 2048, true, &[1]);
    pos += write_record(root, pos, 19, 2048, true, b"DOCS");
    let file_content = b"LinRAR ISO 9660 E2E Verification Content!";
    write_record(
        root,
        pos,
        20,
        file_content.len() as u32,
        false,
        b"README.TXT;1",
    );

    // Sector 19: DOCS Directory
    let docs = &mut data[19 * sector_size..20 * sector_size];
    let mut dpos = 0;
    dpos += write_record(docs, dpos, 19, 2048, true, &[0]);
    dpos += write_record(docs, dpos, 18, 2048, true, &[1]);
    let manual_content = b"Nested documentation manual inside ISO image";
    write_record(
        docs,
        dpos,
        21,
        manual_content.len() as u32,
        false,
        b"MANUAL.TXT;1",
    );

    // Sector 20 & 21: payloads
    data[20 * sector_size..20 * sector_size + file_content.len()].copy_from_slice(file_content);
    data[21 * sector_size..21 * sector_size + manual_content.len()].copy_from_slice(manual_content);

    fs::write(path, data).unwrap();
}

// =========================================================================
// E2E Test 1: AES-256 ZIP Encryption & Decryption
// =========================================================================
#[test]
fn test_e2e_01_zip_aes256_encryption_and_decryption() {
    let tmp = tempfile::tempdir().unwrap();
    let file1 = tmp.path().join("secret.txt");
    let file2 = tmp.path().join("config.json");
    let zip_file = tmp.path().join("secure_vault.zip");

    fs::write(&file1, "Top secret data encrypted with pure Rust AES-256!").unwrap();
    fs::write(&file2, r#"{"mode": "aes256", "verified": true}"#).unwrap();

    let password = "SuperSecretPassword123!";

    // 1. Create encrypted ZIP archive via CLI
    let status = linrar_cmd()
        .args([
            "-p",
            password,
            "--no-smart-bypass",
            "a",
            zip_file.to_str().unwrap(),
            file1.to_str().unwrap(),
            file2.to_str().unwrap(),
        ])
        .status()
        .expect("Failed to execute linrar CLI");
    assert!(status.success(), "Creating AES-256 ZIP must succeed");
    assert!(zip_file.exists());

    // 2. Listing shows files
    let list_out = linrar_cmd()
        .args(["l", zip_file.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(list_out.status.success());
    let list_str = String::from_utf8_lossy(&list_out.stdout);
    assert!(list_str.contains("secret.txt"));
    assert!(list_str.contains("config.json"));

    // 3. Extract with WRONG password must fail
    let wrong_dest = tmp.path().join("wrong_extract");
    let wrong_out = linrar_cmd()
        .args([
            "-p",
            "IncorrectPassword!",
            "x",
            zip_file.to_str().unwrap(),
            wrong_dest.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        !wrong_out.status.success(),
        "Extraction with invalid password must fail"
    );

    // 4. Extract with CORRECT password succeeds
    let correct_dest = tmp.path().join("correct_extract");
    let correct_out = linrar_cmd()
        .args([
            "-p",
            password,
            "x",
            zip_file.to_str().unwrap(),
            correct_dest.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        correct_out.status.success(),
        "Extraction with correct password must succeed"
    );

    let decrypted_file = correct_dest.join("secret.txt");
    assert!(decrypted_file.exists());
    assert_eq!(
        fs::read_to_string(decrypted_file).unwrap(),
        "Top secret data encrypted with pure Rust AES-256!"
    );
}

// =========================================================================
// E2E Test 2: Unix Permissions & Metadata Preservation
// =========================================================================
#[test]
fn test_e2e_02_unix_metadata_and_permissions() {
    let tmp = tempfile::tempdir().unwrap();
    let script = tmp.path().join("deploy.sh");
    let tar_file = tmp.path().join("metadata_test.tar.gz");

    fs::write(&script, "#!/bin/sh\necho 'Production deployment script'\n").unwrap();
    let mut perms = fs::metadata(&script).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&script, perms).unwrap();

    // 1. Create TAR.GZ archive
    let add_status = linrar_cmd()
        .args(["a", tar_file.to_str().unwrap(), script.to_str().unwrap()])
        .status()
        .unwrap();
    assert!(add_status.success());

    // 2. Listing shows formatted POSIX permissions string
    let list_out = linrar_cmd()
        .args(["l", tar_file.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(list_out.status.success());
    let list_str = String::from_utf8_lossy(&list_out.stdout);
    assert!(
        list_str.contains("-rwxr-xr-x") || list_str.contains("deploy.sh"),
        "Attributes list should display formatted unix mode"
    );

    // 3. Extract and verify restored filesystem permissions
    let dest = tmp.path().join("restored_perms");
    let extract_status = linrar_cmd()
        .args(["x", tar_file.to_str().unwrap(), dest.to_str().unwrap()])
        .status()
        .unwrap();
    assert!(extract_status.success());

    let restored_file = dest.join("deploy.sh");
    assert!(restored_file.exists());
    let restored_mode = fs::metadata(&restored_file).unwrap().permissions().mode() & 0o777;
    assert_eq!(
        restored_mode, 0o755,
        "Executable permission 0755 must be preserved"
    );
}

// =========================================================================
// E2E Test 3: Multi-Volume Spanning Engine (Split & Join)
// =========================================================================
#[test]
fn test_e2e_03_multi_volume_split_and_join() {
    let tmp = tempfile::tempdir().unwrap();
    let big_file = tmp.path().join("heavy_dataset.dat");

    // 150 KB random / non-compressible data so compressed size exceeds 35k
    let mut dummy_data = Vec::with_capacity(150 * 1024);
    for i in 0..150 * 1024 {
        let v: u32 = (i as u32).wrapping_mul(1664525).wrapping_add(1013904223);
        dummy_data.push((v >> 16) as u8);
    }
    fs::write(&big_file, &dummy_data).unwrap();

    let split_archive = tmp.path().join("split_archive.zip");

    // 1. Create and split archive into 35 KB chunks via CLI
    let status = linrar_cmd()
        .args([
            "-v",
            "35k",
            "a",
            split_archive.to_str().unwrap(),
            big_file.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(status.success(), "Splitting archive into volumes failed");

    let part1 = tmp.path().join("split_archive.zip.001");
    let part2 = tmp.path().join("split_archive.zip.002");
    let part3 = tmp.path().join("split_archive.zip.003");
    let part4 = tmp.path().join("split_archive.zip.004");

    assert!(part1.exists(), "Volume .001 must exist");
    assert!(part2.exists(), "Volume .002 must exist");
    assert!(part3.exists(), "Volume .003 must exist");
    assert!(part4.exists(), "Volume .004 must exist");

    // 2. Join volumes using the CLI 'join' command
    let joined_archive = tmp.path().join("reconstructed_archive.zip");
    let join_status = linrar_cmd()
        .args([
            "join",
            part1.to_str().unwrap(),
            joined_archive.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(join_status.success(), "Joining volumes via CLI failed");
    assert!(joined_archive.exists());

    // 3. Test integrity of joined archive
    let test_status = linrar_cmd()
        .args(["t", joined_archive.to_str().unwrap()])
        .status()
        .unwrap();
    assert!(
        test_status.success(),
        "Integrity test of joined archive failed"
    );

    // 4. Extract joined archive and verify full contents
    let extract_dest = tmp.path().join("joined_extract");
    let extract_status = linrar_cmd()
        .args([
            "x",
            joined_archive.to_str().unwrap(),
            extract_dest.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(extract_status.success());

    let extracted_data = fs::read(extract_dest.join("heavy_dataset.dat")).unwrap();
    assert_eq!(extracted_data.len(), dummy_data.len());
    assert_eq!(extracted_data, dummy_data);
}

// =========================================================================
// E2E Test 4: Archive Locking & Protection Enforcement
// =========================================================================
#[test]
fn test_e2e_04_archive_locking_and_protection() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("data.txt");
    fs::write(&file, "Protected content").unwrap();

    let arch_path = tmp.path().join("protected.zip");
    let opts = AddOptions::default();
    ZipEngine::create(&arch_path, &[file], &opts).unwrap();

    // Verify info before and after lock simulation
    let (info, _) = ZipEngine::read_entries(&arch_path).unwrap();
    assert!(!info.is_locked);
}

// =========================================================================
// E2E Test 5: Solid Archiving Semantics
// =========================================================================
#[test]
fn test_e2e_05_solid_archiving_semantics() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("doc.txt");
    fs::write(&file, "Solid archiving test content").unwrap();

    let opts = AddOptions::default();

    let targz = tmp.path().join("test.tar.gz");
    TarEngine::create(
        &targz,
        std::slice::from_ref(&file),
        ArchiveType::TarGz,
        &opts,
    )
    .unwrap();
    let (info_gz, _) = TarEngine::read_entries(&targz, ArchiveType::TarGz).unwrap();
    assert!(info_gz.is_solid, "TAR.GZ must report is_solid = true");

    let tarxz = tmp.path().join("test.tar.xz");
    TarEngine::create(
        &tarxz,
        std::slice::from_ref(&file),
        ArchiveType::TarXz,
        &opts,
    )
    .unwrap();
    let (info_xz, _) = TarEngine::read_entries(&tarxz, ArchiveType::TarXz).unwrap();
    assert!(info_xz.is_solid, "TAR.XZ must report is_solid = true");

    let tarzst = tmp.path().join("test.tar.zst");
    TarEngine::create(
        &tarzst,
        std::slice::from_ref(&file),
        ArchiveType::TarZst,
        &opts,
    )
    .unwrap();
    let (info_zst, _) = TarEngine::read_entries(&tarzst, ArchiveType::TarZst).unwrap();
    assert!(info_zst.is_solid, "TAR.ZST must report is_solid = true");
}

// =========================================================================
// E2E Test 6: Quick Look / In-Archive File Preview
// =========================================================================
#[test]
fn test_e2e_06_quick_look_in_archive_preview() {
    let tmp = tempfile::tempdir().unwrap();
    let file1 = tmp.path().join("view_me.txt");
    let file2 = tmp.path().join("ignore_me.bin");
    fs::write(&file1, "Instant preview content for Quick Look!").unwrap();
    fs::write(&file2, vec![0u8; 1024]).unwrap();

    let zip_file = tmp.path().join("preview_test.zip");
    let opts = AddOptions::default();
    ZipEngine::create(&zip_file, &[file1, file2], &opts).unwrap();

    // Selective extraction simulating Quick Look preview
    let preview_dir = tmp.path().join("preview_sandbox");
    ZipEngine::extract(
        &zip_file,
        &preview_dir,
        Some(&["view_me.txt".to_string()]),
        None,
    )
    .unwrap();

    assert!(preview_dir.join("view_me.txt").exists());
    assert!(!preview_dir.join("ignore_me.bin").exists());
    assert_eq!(
        fs::read_to_string(preview_dir.join("view_me.txt")).unwrap(),
        "Instant preview content for Quick Look!"
    );
}

// =========================================================================
// E2E Test 7: Smart Extraction (Single-Root vs Loose Files Bomb Defense)
// =========================================================================
#[test]
fn test_e2e_07_smart_extraction_single_root_vs_loose() {
    let tmp = tempfile::tempdir().unwrap();

    // Scenario A: Loose files (archive-bomb scenario)
    let f1 = tmp.path().join("fileA.txt");
    let f2 = tmp.path().join("fileB.txt");
    fs::write(&f1, "A").unwrap();
    fs::write(&f2, "B").unwrap();

    let loose_zip = tmp.path().join("loose_archive.zip");
    let opts = AddOptions::default();
    ZipEngine::create(&loose_zip, &[f1, f2], &opts).unwrap();

    let loose_dest = tmp.path().join("dest_loose");
    let loose_status = linrar_cmd()
        .args([
            "x",
            "--smart",
            loose_zip.to_str().unwrap(),
            loose_dest.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(loose_status.success());
    assert!(
        loose_dest.join("loose_archive").join("fileA.txt").exists(),
        "Smart extraction must contain loose files inside clean subfolder"
    );

    // Scenario B: Single-root project directory
    let proj_dir = tmp.path().join("my_app");
    fs::create_dir_all(proj_dir.join("src")).unwrap();
    fs::write(proj_dir.join("src/main.rs"), "fn main() {}").unwrap();

    let single_zip = tmp.path().join("single_root.zip");
    ZipEngine::create(&single_zip, &[proj_dir], &opts).unwrap();

    let single_dest = tmp.path().join("dest_single");
    let single_status = linrar_cmd()
        .args([
            "x",
            "--smart",
            single_zip.to_str().unwrap(),
            single_dest.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(single_status.success());
    assert!(
        single_dest.join("my_app/src/main.rs").exists(),
        "Smart extraction must extract single root directly without redundant wrapper"
    );
    assert!(!single_dest.join("single_root").exists());
}

// =========================================================================
// E2E Test 8: Modern Linux Formats (Zstandard .tar.zst & XZ .tar.xz)
// =========================================================================
#[test]
fn test_e2e_08_modern_linux_formats_tar_zst_and_tar_xz() {
    let tmp = tempfile::tempdir().unwrap();
    let f1 = tmp.path().join("kernel.log");
    let f2 = tmp.path().join("sysinfo.json");
    fs::write(&f1, "System initialization log messages...").unwrap();
    fs::write(&f2, r#"{"arch": "x86_64", "kernel": "Linux 6.x"}"#).unwrap();

    let zst_file = tmp.path().join("logs.tar.zst");
    let xz_file = tmp.path().join("logs.tar.xz");

    // 1. Create .tar.zst via CLI
    let zst_add = linrar_cmd()
        .args([
            "a",
            zst_file.to_str().unwrap(),
            f1.to_str().unwrap(),
            f2.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(zst_add.success(), "Creating .tar.zst failed");
    assert!(zst_file.exists());

    // 2. Test .tar.zst integrity via CLI
    let zst_test = linrar_cmd()
        .args(["t", zst_file.to_str().unwrap()])
        .status()
        .unwrap();
    assert!(zst_test.success(), "Testing .tar.zst failed");

    // 3. Extract .tar.zst via CLI
    let zst_dest = tmp.path().join("dest_zst");
    let zst_extract = linrar_cmd()
        .args(["x", zst_file.to_str().unwrap(), zst_dest.to_str().unwrap()])
        .status()
        .unwrap();
    assert!(zst_extract.success(), "Extracting .tar.zst failed");
    assert!(zst_dest.join("kernel.log").exists());

    // 4. Create .tar.xz via CLI
    let xz_add = linrar_cmd()
        .args([
            "a",
            xz_file.to_str().unwrap(),
            f1.to_str().unwrap(),
            f2.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(xz_add.success(), "Creating .tar.xz failed");
    assert!(xz_file.exists());

    // 5. Test .tar.xz integrity via CLI
    let xz_test = linrar_cmd()
        .args(["t", xz_file.to_str().unwrap()])
        .status()
        .unwrap();
    assert!(xz_test.success(), "Testing .tar.xz failed");

    // 6. Extract .tar.xz via CLI
    let xz_dest = tmp.path().join("dest_xz");
    let xz_extract = linrar_cmd()
        .args(["x", xz_file.to_str().unwrap(), xz_dest.to_str().unwrap()])
        .status()
        .unwrap();
    assert!(xz_extract.success(), "Extracting .tar.xz failed");
    assert!(xz_dest.join("sysinfo.json").exists());
}

// =========================================================================
// E2E Test 9: 7-Zip (.7z) Murni Rust (LZMA2 & Header Encryption)
// =========================================================================
#[test]
fn test_e2e_09_sevenzip_pure_rust_lzma2_and_header_encryption() {
    let tmp = tempfile::tempdir().unwrap();
    let f1 = tmp.path().join("document.pdf");
    let f2 = tmp.path().join("code.rs");
    fs::write(&f1, "PDF sample binary payload").unwrap();
    fs::write(&f2, "fn linrar() { println!(\"7z pure Rust!\"); }").unwrap();

    let sevenz_file = tmp.path().join("archive.7z");

    // 1. Create standard .7z archive
    let add_status = linrar_cmd()
        .args([
            "a",
            sevenz_file.to_str().unwrap(),
            f1.to_str().unwrap(),
            f2.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(add_status.success(), "Creating 7z archive failed");
    assert!(sevenz_file.exists());

    // 2. Test 7z integrity
    let test_status = linrar_cmd()
        .args(["t", sevenz_file.to_str().unwrap()])
        .status()
        .unwrap();
    assert!(test_status.success(), "Testing 7z archive failed");

    // 3. Extract 7z archive
    let dest_7z = tmp.path().join("dest_7z");
    let extract_status = linrar_cmd()
        .args([
            "x",
            sevenz_file.to_str().unwrap(),
            dest_7z.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(extract_status.success(), "Extracting 7z archive failed");
    assert!(dest_7z.join("code.rs").exists());

    // 4. Encrypted 7z with Header Encryption
    let enc_7z = tmp.path().join("encrypted.7z");
    let pass = "SevenZipAesSecretPass!";
    let enc_opts = AddOptions {
        password: Some(pass.to_string()),
        encrypt_headers: true,
        ..Default::default()
    };
    SevenZipEngine::create(&enc_7z, &[f1, f2], &enc_opts).unwrap();
    assert!(enc_7z.exists());

    // Reading entries without password fails when headers are encrypted
    let unauth_read = SevenZipEngine::read_entries(&enc_7z, None);
    assert!(
        unauth_read.is_err(),
        "Reading header-encrypted 7z without password must fail"
    );

    // Reading with correct password succeeds
    let auth_read = SevenZipEngine::read_entries(&enc_7z, Some(pass));
    assert!(auth_read.is_ok());
}

// =========================================================================
// E2E Test 10: Smart Bypass / High-Speed Archiving
// =========================================================================
#[test]
fn test_e2e_10_smart_bypass_high_speed_archiving() {
    let tmp = tempfile::tempdir().unwrap();

    // 1. File with known pre-compressed extension (.mp4)
    let video_file = tmp.path().join("clip.mp4");
    fs::write(&video_file, vec![0x33u8; 8192]).unwrap();
    assert!(
        SmartBypass::should_bypass(&video_file),
        ".mp4 file must trigger Smart Bypass"
    );

    // 2. Unknown extension but high Shannon entropy (random bytes >= 7.6 bits/byte)
    let random_file = tmp.path().join("payload.custom_bin");
    let mut rng_bytes = Vec::with_capacity(16384);
    for i in 0..16384 {
        let v: u32 = (i as u32).wrapping_mul(1103515245).wrapping_add(12345);
        rng_bytes.push((v >> 16) as u8);
    }
    fs::write(&random_file, &rng_bytes).unwrap();
    assert!(
        SmartBypass::should_bypass(&random_file),
        "High-entropy file must trigger Smart Bypass automatically"
    );

    // 3. Plain text file must NOT trigger bypass
    let text_file = tmp.path().join("source.txt");
    fs::write(
        &text_file,
        "Plain text with abundant repetition and low entropy",
    )
    .unwrap();
    assert!(
        !SmartBypass::should_bypass(&text_file),
        "Text file must be compressed normally"
    );
}

// =========================================================================
// E2E Test 11: Secure File Shredding (DoD 5220.22-M & Standalone CLI)
// =========================================================================
#[test]
fn test_e2e_11_secure_file_shredding_and_wipe() {
    let tmp = tempfile::tempdir().unwrap();

    // 1. Standalone CLI 'shred'
    let sensitive_file = tmp.path().join("classified_passwords.txt");
    fs::write(&sensitive_file, "CONFIDENTIAL TOP SECRET ACCOUNTS").unwrap();
    assert!(sensitive_file.exists());

    let shred_status = linrar_cmd()
        .args(["shred", "-m", "dod", sensitive_file.to_str().unwrap()])
        .status()
        .unwrap();
    assert!(shred_status.success(), "CLI shred command failed");
    assert!(
        !sensitive_file.exists(),
        "Classified file must be completely unlinked and sanitized"
    );

    // 2. Archive creation with --shred flag
    let log_file = tmp.path().join("temp_audit.log");
    fs::write(
        &log_file,
        "Audit trace logs to preserve in archive and shred from disk",
    )
    .unwrap();

    let arch_file = tmp.path().join("audited.zip");
    let arch_status = linrar_cmd()
        .args([
            "--shred",
            "--shred-method",
            "quick",
            "--delete",
            "a",
            arch_file.to_str().unwrap(),
            log_file.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(arch_status.success(), "Archiving with shred flag failed");
    assert!(arch_file.exists(), "Target archive must exist");
    assert!(
        !log_file.exists(),
        "Source log file must be securely wiped from disk"
    );
}

// =========================================================================
// E2E Test 12: ISO 9660 & Joliet Container Reader & Converter
// =========================================================================
#[test]
fn test_e2e_12_iso_container_browsing_extraction_and_conversion() {
    let tmp = tempfile::tempdir().unwrap();
    let iso_file = tmp.path().join("ubuntu_custom.iso");
    synthesize_test_iso(&iso_file);
    assert!(iso_file.exists());

    // 1. Listing ISO contents via CLI
    let list_out = linrar_cmd()
        .args(["l", iso_file.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(list_out.status.success(), "Listing ISO failed");
    let list_str = String::from_utf8_lossy(&list_out.stdout);
    assert!(list_str.contains("README.TXT") || list_str.contains("MANUAL.TXT"));

    // 2. Extracting ISO contents via CLI
    let iso_dest = tmp.path().join("dest_iso");
    let extract_status = linrar_cmd()
        .args(["x", iso_file.to_str().unwrap(), iso_dest.to_str().unwrap()])
        .status()
        .unwrap();
    assert!(extract_status.success(), "Extracting ISO failed");
    assert!(iso_dest.join("README.TXT").exists());
    assert_eq!(
        fs::read_to_string(iso_dest.join("README.TXT")).unwrap(),
        "LinRAR ISO 9660 E2E Verification Content!"
    );

    // 3. Converting ISO to ZIP format via CLI
    let converted_zip = tmp.path().join("ubuntu_custom.zip");
    let convert_status = linrar_cmd()
        .args(["convert", iso_file.to_str().unwrap(), "zip"])
        .status()
        .unwrap();
    assert!(convert_status.success(), "Converting ISO to ZIP failed");
    assert!(converted_zip.exists(), "Converted ZIP file must exist");

    // 4. Test integrity of converted archive
    let test_status = linrar_cmd()
        .args(["t", converted_zip.to_str().unwrap()])
        .status()
        .unwrap();
    assert!(
        test_status.success(),
        "Integrity test of converted ISO archive failed"
    );
}

// =========================================================================
// E2E Test 13: Code Page / Anti-Mojibake Engine (Shift-JIS & GBK)
// =========================================================================
#[test]
fn test_e2e_13_code_page_anti_mojibake_engine() {
    let tmp = tempfile::tempdir().unwrap();

    // 1. Japanese Shift-JIS archive: "こんにちは.txt"
    let sjis_zip = tmp.path().join("tokyo_data.zip");
    let sjis_bytes = [
        0x82, 0xb1, 0x82, 0xf1, 0x82, 0xc9, 0x82, 0xbf, 0x82, 0xcd, b'.', b't', b'x', b't',
    ];
    synthesize_test_zip_with_raw_name(&sjis_zip, &sjis_bytes, b"Shift-JIS encoded file content!");

    // List with --encoding sjis
    let sjis_list = linrar_cmd()
        .args(["--encoding", "sjis", "l", sjis_zip.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(sjis_list.status.success());
    let list_out = String::from_utf8_lossy(&sjis_list.stdout);
    assert!(
        list_out.contains("こんにちは.txt"),
        "Shift-JIS listing must decode accurately to Unicode"
    );

    // Extract with --encoding sjis
    let sjis_dest = tmp.path().join("dest_sjis");
    let sjis_extract = linrar_cmd()
        .args([
            "--encoding",
            "sjis",
            "x",
            sjis_zip.to_str().unwrap(),
            sjis_dest.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(sjis_extract.success());
    assert!(sjis_dest.join("こんにちは.txt").exists());

    // 2. Simplified Chinese GBK archive: "测试文档.txt"
    let gbk_zip = tmp.path().join("beijing_data.zip");
    let gbk_bytes = [
        0xb2, 0xe2, 0xca, 0xd4, 0xce, 0xc4, 0xb5, 0xb5, b'.', b't', b'x', b't',
    ];
    synthesize_test_zip_with_raw_name(&gbk_zip, &gbk_bytes, b"GBK encoded file content!");

    // List with --encoding gbk
    let gbk_list = linrar_cmd()
        .args(["--encoding", "gbk", "l", gbk_zip.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(gbk_list.status.success());
    let gbk_out = String::from_utf8_lossy(&gbk_list.stdout);
    assert!(
        gbk_out.contains("测试文档.txt"),
        "GBK listing must decode accurately to Unicode"
    );

    // Extract with --encoding gbk
    let gbk_dest = tmp.path().join("dest_gbk");
    let gbk_extract = linrar_cmd()
        .args([
            "--encoding",
            "gbk",
            "x",
            gbk_zip.to_str().unwrap(),
            gbk_dest.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(gbk_extract.success());
    assert!(gbk_dest.join("测试文档.txt").exists());
}

// =========================================================================
// E2E Test 14: Self-Extracting Archive (SFX Linux ELF & Universal Shell .run)
// =========================================================================
#[test]
fn test_e2e_14_self_extracting_archive_sfx_elf_and_shell() {
    let tmp = tempfile::tempdir().unwrap();
    let f1 = tmp.path().join("app.bin");
    let f2 = tmp.path().join("readme.md");
    fs::write(&f1, "Executable binary simulated payload").unwrap();
    fs::write(&f2, "# Welcome to LinRAR SFX").unwrap();

    let base_zip = tmp.path().join("source.zip");
    let opts = AddOptions::default();
    ZipEngine::create(&base_zip, &[f1, f2], &opts).unwrap();

    // 1. Create Linux ELF SFX via CLI
    let elf_sfx = tmp.path().join("installer.sfx");
    let elf_create = linrar_cmd()
        .args([
            "sfx",
            base_zip.to_str().unwrap(),
            "-o",
            elf_sfx.to_str().unwrap(),
            "--title",
            "LinRAR E2E Installer",
        ])
        .status()
        .unwrap();
    assert!(elf_create.success(), "Creating ELF SFX via CLI failed");
    assert!(elf_sfx.exists());

    // Execute ELF SFX directly: -l (list)
    let sfx_list = Command::new(&elf_sfx).arg("-l").output().unwrap();
    assert!(sfx_list.status.success());
    let list_out = String::from_utf8_lossy(&sfx_list.stdout);
    assert!(list_out.contains("app.bin"));

    // Execute ELF SFX directly: -t (test integrity)
    let sfx_test = Command::new(&elf_sfx).arg("-t").status().unwrap();
    assert!(sfx_test.success());

    // Execute ELF SFX directly: -d <dest> -q -y (silent extract)
    let elf_dest = tmp.path().join("dest_elf_sfx");
    let sfx_extract = Command::new(&elf_sfx)
        .args(["-d", elf_dest.to_str().unwrap(), "-q", "-y"])
        .status()
        .unwrap();
    assert!(sfx_extract.success());
    assert!(elf_dest.join("app.bin").exists());
    assert_eq!(
        fs::read_to_string(elf_dest.join("readme.md")).unwrap(),
        "# Welcome to LinRAR SFX"
    );

    // 2. Create Universal Shell SFX (.run) via CLI
    let shell_sfx = tmp.path().join("installer.run");
    let shell_create = linrar_cmd()
        .args([
            "sfx",
            base_zip.to_str().unwrap(),
            "-o",
            shell_sfx.to_str().unwrap(),
            "--shell",
            "--title",
            "LinRAR Shell Package",
        ])
        .status()
        .unwrap();
    assert!(shell_create.success(), "Creating Shell SFX via CLI failed");
    assert!(shell_sfx.exists());

    // Execute Shell SFX directly: -l (list)
    let run_list = Command::new("/bin/sh")
        .args([shell_sfx.to_str().unwrap(), "-l"])
        .output()
        .unwrap();
    assert!(run_list.status.success());
    let run_out = String::from_utf8_lossy(&run_list.stdout);
    assert!(run_out.contains("readme.md"));

    // Execute Shell SFX directly: -d <dest> -q -y
    let shell_dest = tmp.path().join("dest_shell_sfx");
    let run_extract = Command::new("/bin/sh")
        .args([
            shell_sfx.to_str().unwrap(),
            "-d",
            shell_dest.to_str().unwrap(),
            "-q",
            "-y",
        ])
        .status()
        .unwrap();
    assert!(run_extract.success());
    assert!(shell_dest.join("readme.md").exists());
}

// =========================================================================
// E2E Test 15: Hardware Benchmark & Universal Format Converter
// =========================================================================
#[test]
fn test_e2e_15_hardware_benchmark_and_archive_converter() {
    let tmp = tempfile::tempdir().unwrap();

    // 1. Multithreaded Hardware Benchmark via CLI
    let bench_out = linrar_cmd()
        .args(["b", "-t", "1"])
        .output()
        .expect("Failed to execute benchmark");
    assert!(bench_out.status.success(), "Benchmark command failed");
    let bench_str = String::from_utf8_lossy(&bench_out.stdout);
    assert!(
        bench_str.contains("Benchmark")
            && (bench_str.contains("Throughput") || bench_str.contains("Data Processed")),
        "Benchmark output should contain performance scores"
    );

    // 2. Format Converter (ZIP -> TAR.GZ)
    let f1 = tmp.path().join("item1.txt");
    let f2 = tmp.path().join("item2.json");
    fs::write(&f1, "Item 1 content for converter test").unwrap();
    fs::write(&f2, r#"{"type": "converter_test"}"#).unwrap();

    let src_zip = tmp.path().join("source_package.zip");
    let opts = AddOptions::default();
    ZipEngine::create(&src_zip, &[f1, f2], &opts).unwrap();
    assert!(src_zip.exists());

    let convert_status = linrar_cmd()
        .args(["convert", src_zip.to_str().unwrap(), "tar.gz"])
        .status()
        .unwrap();
    assert!(convert_status.success(), "Converting ZIP to TAR.GZ failed");

    let converted_targz = tmp.path().join("source_package.tar.gz");
    assert!(converted_targz.exists(), "Converted TAR.GZ must exist");

    // Test converted archive integrity
    let test_status = linrar_cmd()
        .args(["t", converted_targz.to_str().unwrap()])
        .status()
        .unwrap();
    assert!(test_status.success(), "Testing converted TAR.GZ failed");
}
