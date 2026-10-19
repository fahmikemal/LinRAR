use std::fs;
use std::path::PathBuf;

use crate::core::benchmark::BenchmarkRunner;
use crate::core::bypass::SmartBypass;
use crate::core::converter::ArchiveConverter;
use crate::core::detector::detect_archive_type;
use crate::core::finder::ArchiveFinder;
use crate::core::iso_engine::IsoEngine;
use crate::core::rar_engine::RarEngine;
use crate::core::report::{ArchiveReport, ReportFormat};
use crate::core::scanner::VirusScanner;
use crate::core::sevenzip_engine::SevenZipEngine;
use crate::core::shredder::{FileShredder, ShredMethod};
use crate::core::smart_extract::SmartExtractor;
use crate::core::tar_engine::TarEngine;
use crate::core::volume::VolumeManager;
use crate::core::zip_engine::ZipEngine;
use crate::types::{AddOptions, ArchiveEntry, ArchiveType, CompressionLevel, format_unix_mode};

fn setup_test_files() -> (tempfile::TempDir, Vec<PathBuf>) {
    let tmp = tempfile::tempdir().unwrap();
    let file1 = tmp.path().join("hello.txt");
    let file2 = tmp.path().join("data.json");
    let subfolder = tmp.path().join("subfolder");
    fs::create_dir_all(&subfolder).unwrap();
    let file3 = subfolder.join("nested.txt");

    fs::write(&file1, "Hello from LinRAR Pure Rust!").unwrap();
    fs::write(
        &file2,
        r#"{"app": "LinRAR", "version": "0.1.0", "language": "Rust"}"#,
    )
    .unwrap();
    fs::write(&file3, "Nested content for directory testing").unwrap();

    let sources = vec![file1, file2, subfolder];
    (tmp, sources)
}

#[test]
fn test_magic_detection() {
    let (tmp, sources) = setup_test_files();
    let zip_path = tmp.path().join("test.zip");
    let targz_path = tmp.path().join("test.tar.gz");
    let tarxz_path = tmp.path().join("test.tar.xz");
    let tarzst_path = tmp.path().join("test.tar.zst");
    let sevenz_path = tmp.path().join("test.7z");

    let opts = AddOptions::default();
    ZipEngine::create(&zip_path, &sources, &opts).unwrap();
    TarEngine::create(&targz_path, &sources, ArchiveType::TarGz, &opts).unwrap();
    TarEngine::create(&tarxz_path, &sources, ArchiveType::TarXz, &opts).unwrap();
    TarEngine::create(&tarzst_path, &sources, ArchiveType::TarZst, &opts).unwrap();
    SevenZipEngine::create(&sevenz_path, &sources, &opts).unwrap();

    assert_eq!(detect_archive_type(&zip_path), ArchiveType::Zip);
    assert_eq!(detect_archive_type(&targz_path), ArchiveType::TarGz);
    assert_eq!(detect_archive_type(&tarxz_path), ArchiveType::TarXz);
    assert_eq!(detect_archive_type(&tarzst_path), ArchiveType::TarZst);
    assert_eq!(detect_archive_type(&sevenz_path), ArchiveType::SevenZip);
}

#[test]
fn test_zip_lifecycle() {
    let (tmp, sources) = setup_test_files();
    let zip_path = tmp.path().join("test_lifecycle.zip");
    let extract_dir = tmp.path().join("zip_out");

    let opts = AddOptions {
        compression_level: CompressionLevel::Best,
        ..Default::default()
    };

    // Create
    ZipEngine::create(&zip_path, &sources, &opts).unwrap();
    assert!(zip_path.exists());

    // Read entries
    let (info, entries) = ZipEngine::read_entries(&zip_path).unwrap();
    assert_eq!(info.archive_type, ArchiveType::Zip);
    assert!(entries.iter().any(|e| e.name == "hello.txt"));

    // Extract
    ZipEngine::extract(&zip_path, &extract_dir, None, None).unwrap();
    assert_eq!(
        fs::read_to_string(extract_dir.join("hello.txt")).unwrap(),
        "Hello from LinRAR Pure Rust!"
    );
}

#[test]
fn test_tar_lifecycle() {
    let (tmp, sources) = setup_test_files();
    let tar_path = tmp.path().join("test_lifecycle.tar.gz");
    let extract_dir = tmp.path().join("tar_out");

    let opts = AddOptions::default();
    TarEngine::create(&tar_path, &sources, ArchiveType::TarGz, &opts).unwrap();

    let (info, entries) = TarEngine::read_entries(&tar_path, ArchiveType::TarGz).unwrap();
    assert_eq!(info.archive_type, ArchiveType::TarGz);
    assert!(entries.iter().any(|e| e.name == "hello.txt"));

    TarEngine::extract(&tar_path, &extract_dir, ArchiveType::TarGz, None).unwrap();
    assert_eq!(
        fs::read_to_string(extract_dir.join("hello.txt")).unwrap(),
        "Hello from LinRAR Pure Rust!"
    );
}

#[test]
fn test_rar_lifecycle() {
    if RarEngine::get_rar_bin().is_none() {
        println!("rar binary not found in PATH, skipping rar lifecycle test");
        return;
    }

    let (tmp, sources) = setup_test_files();
    let rar_path = tmp.path().join("test_lifecycle.rar");
    let extract_dir = tmp.path().join("rar_out");

    let opts = AddOptions {
        compression_level: CompressionLevel::Normal,
        ..Default::default()
    };

    // Create RAR
    RarEngine::create(&rar_path, &sources, &opts).unwrap();
    assert!(rar_path.exists());

    // Test integrity
    assert!(RarEngine::test(&rar_path).unwrap());

    // Read entries
    let (info, entries) = RarEngine::read_entries(&rar_path).unwrap();
    assert_eq!(info.archive_type, ArchiveType::Rar5);
    assert!(entries.iter().any(|e| e.name == "hello.txt"));

    // Extract
    RarEngine::extract(&rar_path, &extract_dir, None, None).unwrap();
    assert_eq!(
        fs::read_to_string(extract_dir.join("hello.txt")).unwrap(),
        "Hello from LinRAR Pure Rust!"
    );

    // Delete
    RarEngine::delete(&rar_path, &["hello.txt".to_string()]).unwrap();
    let (_, remaining_entries) = RarEngine::read_entries(&rar_path).unwrap();
    assert!(!remaining_entries.iter().any(|e| e.name == "hello.txt"));
}

#[test]
fn test_benchmark_runner() {
    let mut runner = BenchmarkRunner::new();
    runner.start();
    std::thread::sleep(std::time::Duration::from_millis(500));
    runner.stop();

    assert!(runner.current_speed_kb_s() > 0.0);
    assert!(runner.total_processed_mb() > 0.0);
    assert!(runner.elapsed_seconds() >= 0.4);
}

#[test]
fn test_virus_scanner() {
    let tmp = tempfile::tempdir().unwrap();
    let clean_file = tmp.path().join("clean.txt");
    fs::write(&clean_file, "This is clean text.").unwrap();

    let clean_zip = tmp.path().join("clean.zip");
    ZipEngine::create(&clean_zip, &[clean_file], &AddOptions::default()).unwrap();

    let clean_res = VirusScanner::scan_archive(&clean_zip).unwrap();
    assert!(clean_res.clean);
    assert_eq!(clean_res.threats.len(), 0);

    // Test EICAR pattern detection
    let infected_file = tmp.path().join("eicar.com");
    fs::write(
        &infected_file,
        b"X5O!P%@AP[4\\PZX54(P^)7CC)7}$EICAR-STANDARD-ANTIVIRUS-TEST-FILE!$H+H*",
    )
    .unwrap();
    let infected_zip = tmp.path().join("infected.zip");
    ZipEngine::create(&infected_zip, &[infected_file], &AddOptions::default()).unwrap();

    let infected_res = VirusScanner::scan_archive(&infected_zip).unwrap();
    assert!(!infected_res.clean);
    assert!(
        infected_res
            .threats
            .iter()
            .any(|t| t.threat_name.contains("EICAR"))
    );
}

#[test]
fn test_archive_report() {
    let (tmp, sources) = setup_test_files();
    let zip_path = tmp.path().join("report_test.zip");
    ZipEngine::create(&zip_path, &sources, &AddOptions::default()).unwrap();

    let txt_report = ArchiveReport::generate(&zip_path, ReportFormat::Text).unwrap();
    assert!(txt_report.contains("LinRAR 0.1.0 - Archive Contents Report"));
    assert!(txt_report.contains("hello.txt"));
    assert!(txt_report.contains("data.json"));

    let html_report = ArchiveReport::generate(&zip_path, ReportFormat::Html).unwrap();
    assert!(html_report.contains("<!DOCTYPE html>"));
    assert!(html_report.contains("LinRAR 0.1.0 - Archive Contents Report"));
    assert!(html_report.contains("hello.txt"));
}

#[test]
fn test_archive_finder() {
    let (tmp, sources) = setup_test_files();
    let zip_path = tmp.path().join("search_container.zip");
    ZipEngine::create(&zip_path, &sources, &AddOptions::default()).unwrap();

    let matches = ArchiveFinder::find_in_directory(tmp.path(), "hello", true);
    assert!(!matches.is_empty());
    assert!(matches.iter().any(|m| m.file_name == "hello.txt"));
}

#[test]
fn test_archive_converter() {
    let (tmp, sources) = setup_test_files();
    let zip_path = tmp.path().join("convert_src.zip");
    ZipEngine::create(&zip_path, &sources, &AddOptions::default()).unwrap();

    let target =
        ArchiveConverter::convert(&zip_path, ArchiveType::TarGz, &AddOptions::default()).unwrap();

    assert!(target.exists());
    assert_eq!(detect_archive_type(&target), ArchiveType::TarGz);
}

#[test]
fn test_rar_advanced_operations() {
    if RarEngine::get_rar_bin().is_none() {
        println!("rar binary not found in PATH, skipping rar advanced operations test");
        return;
    }

    let (tmp, sources) = setup_test_files();
    let rar_path = tmp.path().join("advanced.rar");
    RarEngine::create(&rar_path, &sources, &AddOptions::default()).unwrap();

    // Protect (Recovery Record)
    assert!(RarEngine::protect(&rar_path, "3%").is_ok());

    // Rename
    assert!(RarEngine::rename(&rar_path, "hello.txt", "renamed_hello.txt").is_ok());
    let (_, entries) = RarEngine::read_entries(&rar_path).unwrap();
    assert!(entries.iter().any(|e| e.name == "renamed_hello.txt"));
    assert!(!entries.iter().any(|e| e.name == "hello.txt"));

    // Set Comment
    assert!(RarEngine::set_comment(&rar_path, "LinRAR Verified Archive").is_ok());
    let (info, _) = RarEngine::read_entries(&rar_path).unwrap();
    assert_eq!(info.comment.as_deref(), Some("LinRAR Verified Archive"));

    // Lock Archive
    assert!(RarEngine::lock(&rar_path).is_ok());
    let (info_locked, _) = RarEngine::read_entries(&rar_path).unwrap();
    assert!(info_locked.is_locked);
}

#[test]
fn test_zip_and_tar_integrity() {
    let (tmp, sources) = setup_test_files();
    let zip_path = tmp.path().join("integrity.zip");
    let targz_path = tmp.path().join("integrity.tar.gz");

    ZipEngine::create(&zip_path, &sources, &AddOptions::default()).unwrap();
    TarEngine::create(
        &targz_path,
        &sources,
        ArchiveType::TarGz,
        &AddOptions::default(),
    )
    .unwrap();

    assert!(ZipEngine::test(&zip_path, None).unwrap());
    assert!(TarEngine::test(&targz_path, ArchiveType::TarGz).unwrap());
}

#[test]
fn test_zip_aes_encryption() {
    let (tmp, sources) = setup_test_files();
    let zip_path = tmp.path().join("encrypted.zip");
    let extract_dir = tmp.path().join("enc_out");

    let pwd = "LinRAR_Strong_Pwd#2026";
    let opts = AddOptions {
        password: Some(pwd.to_string()),
        compression_level: CompressionLevel::Best,
        ..Default::default()
    };

    // Create encrypted ZIP
    ZipEngine::create(&zip_path, &sources, &opts).unwrap();
    assert!(zip_path.exists());

    // Verify metadata shows encrypted
    let (info, entries) = ZipEngine::read_entries(&zip_path).unwrap();
    assert_eq!(info.archive_type, ArchiveType::Zip);
    let hello_entry = entries.iter().find(|e| e.name == "hello.txt").unwrap();
    assert!(hello_entry.encrypted);

    // Testing with wrong password should fail
    assert!(ZipEngine::test(&zip_path, Some("wrong_password")).is_err());

    // Testing with correct password should pass
    assert!(ZipEngine::test(&zip_path, Some(pwd)).unwrap());

    // Extracting with wrong password should fail
    let bad_dir = tmp.path().join("bad_out");
    assert!(ZipEngine::extract(&zip_path, &bad_dir, None, Some("wrong_password")).is_err());

    // Extracting with correct password should succeed and verify content
    ZipEngine::extract(&zip_path, &extract_dir, None, Some(pwd)).unwrap();
    assert_eq!(
        fs::read_to_string(extract_dir.join("hello.txt")).unwrap(),
        "Hello from LinRAR Pure Rust!"
    );
}

#[test]
fn test_volume_manager_split_and_join() {
    // 1. Test size parsing
    assert_eq!(VolumeManager::parse_size("10K").unwrap(), 10 * 1024);
    assert_eq!(VolumeManager::parse_size("5M").unwrap(), 5 * 1024 * 1024);
    assert_eq!(VolumeManager::parse_size("1G").unwrap(), 1024 * 1024 * 1024);
    assert_eq!(VolumeManager::parse_size("1048576").unwrap(), 1048576);
    assert!(VolumeManager::parse_size("invalid").is_err());
    assert!(VolumeManager::parse_size("").is_err());

    // 2. Test split and join
    let tmp = tempfile::tempdir().unwrap();
    let original_file = tmp.path().join("large_payload.bin");

    // Generate 50 KB dummy payload
    let test_data: Vec<u8> = (0..50 * 1024).map(|i| (i % 256) as u8).collect();
    fs::write(&original_file, &test_data).unwrap();

    // Split into 16 KB volumes (should produce 4 chunks: 16k, 16k, 16k, 2k)
    let chunk_size = 16 * 1024;
    let parts = VolumeManager::split_file(&original_file, chunk_size, None).unwrap();
    assert_eq!(parts.len(), 4);
    assert!(parts[0].to_string_lossy().ends_with(".001"));
    assert!(parts[1].to_string_lossy().ends_with(".002"));
    assert!(parts[2].to_string_lossy().ends_with(".003"));
    assert!(parts[3].to_string_lossy().ends_with(".004"));

    // Verify finding volume parts
    let found_parts = VolumeManager::find_volume_parts(&parts[0]).unwrap();
    assert_eq!(found_parts, parts);

    // Reconstruct file
    let joined_file = tmp.path().join("reconstructed_payload.bin");
    VolumeManager::join_files(&parts, &joined_file).unwrap();

    let joined_data = fs::read(&joined_file).unwrap();
    assert_eq!(joined_data, test_data);
}

#[test]
fn test_unix_permissions_formatting() {
    assert_eq!(format_unix_mode(0o755, false), "-rwxr-xr-x");
    assert_eq!(format_unix_mode(0o644, false), "-rw-r--r--");
    assert_eq!(format_unix_mode(0o700, false), "-rwx------");
    assert_eq!(format_unix_mode(0o755, true), "drwxr-xr-x");
    assert_eq!(format_unix_mode(0o600, false), "-rw-------");
}

#[test]
fn test_zip_volume_splitting() {
    let tmp = tempfile::tempdir().unwrap();
    let large_file = tmp.path().join("payload.bin");
    // Generate 10 KB incompressible pseudo-random bytes
    let data: Vec<u8> = (0..10_000).map(|i| ((i * 37 + 13) % 256) as u8).collect();
    fs::write(&large_file, &data).unwrap();
    let sources = vec![large_file];

    let zip_path = tmp.path().join("split_test.zip");
    let opts = AddOptions {
        compression_level: CompressionLevel::Store,
        volume_size: Some("2K".to_string()),
        ..Default::default()
    };

    ZipEngine::create(&zip_path, &sources, &opts).unwrap();

    let first_part = tmp.path().join("split_test.zip.001");
    assert!(first_part.exists());
    let parts = VolumeManager::find_volume_parts(&first_part).unwrap();
    assert!(parts.len() > 1);

    // Reconstruct and verify
    let joined = tmp.path().join("reconstructed.zip");
    VolumeManager::join_files(&parts, &joined).unwrap();
    assert!(joined.exists());
    assert!(ZipEngine::test(&joined, None).unwrap());
}

#[test]
fn test_smart_extractor_stem_cleaner() {
    assert_eq!(
        SmartExtractor::get_clean_archive_stem("myproject.tar.gz"),
        "myproject"
    );
    assert_eq!(
        SmartExtractor::get_clean_archive_stem("backup.tar.bz2"),
        "backup"
    );
    assert_eq!(
        SmartExtractor::get_clean_archive_stem("photos.zip"),
        "photos"
    );
    assert_eq!(
        SmartExtractor::get_clean_archive_stem("large_data.part01.rar"),
        "large_data"
    );
    assert_eq!(
        SmartExtractor::get_clean_archive_stem("multi.zip.001"),
        "multi"
    );
}

#[test]
fn test_smart_extractor_single_root_vs_loose() {
    // 1. Single root folder
    let single_root_entries = vec![
        ArchiveEntry {
            name: "project".to_string(),
            full_path: "project/".to_string(),
            size: 0,
            packed_size: 0,
            modified: "-".to_string(),
            crc: "00000000".to_string(),
            is_dir: true,
            encrypted: false,
            attributes: "drwxr-xr-x".to_string(),
        },
        ArchiveEntry {
            name: "main.rs".to_string(),
            full_path: "project/src/main.rs".to_string(),
            size: 100,
            packed_size: 50,
            modified: "-".to_string(),
            crc: "12345678".to_string(),
            is_dir: false,
            encrypted: false,
            attributes: "-rw-r--r--".to_string(),
        },
    ];
    let (is_single, root) = SmartExtractor::has_single_root_folder(&single_root_entries);
    assert!(is_single);
    assert_eq!(root.as_deref(), Some("project"));

    // 2. Loose files (archive-bomb scenario)
    let loose_entries = vec![
        ArchiveEntry {
            name: "file1.txt".to_string(),
            full_path: "file1.txt".to_string(),
            size: 50,
            packed_size: 20,
            modified: "-".to_string(),
            crc: "AAAAAAAA".to_string(),
            is_dir: false,
            encrypted: false,
            attributes: "-rw-r--r--".to_string(),
        },
        ArchiveEntry {
            name: "file2.txt".to_string(),
            full_path: "file2.txt".to_string(),
            size: 60,
            packed_size: 25,
            modified: "-".to_string(),
            crc: "BBBBBBBB".to_string(),
            is_dir: false,
            encrypted: false,
            attributes: "-rw-r--r--".to_string(),
        },
    ];
    let (is_single_loose, _) = SmartExtractor::has_single_root_folder(&loose_entries);
    assert!(!is_single_loose);

    // 3. Destination resolution
    let base_dest = std::path::Path::new("/home/user/Desktop");
    let dest_single =
        SmartExtractor::determine_destination("myproject.zip", base_dest, &single_root_entries);
    assert_eq!(dest_single, base_dest);

    let dest_loose =
        SmartExtractor::determine_destination("loose_files.zip", base_dest, &loose_entries);
    assert_eq!(dest_loose, base_dest.join("loose_files"));
}

#[test]
fn test_smart_extractor_lifecycle() {
    let tmp = tempfile::tempdir().unwrap();
    let file1 = tmp.path().join("doc1.txt");
    let file2 = tmp.path().join("doc2.txt");
    fs::write(&file1, "First loose file").unwrap();
    fs::write(&file2, "Second loose file").unwrap();

    let loose_zip = tmp.path().join("loose_payload.zip");
    ZipEngine::create(&loose_zip, &[file1, file2], &AddOptions::default()).unwrap();

    let extract_base = tmp.path().join("smart_output");
    let final_dir = SmartExtractor::extract_smart(&loose_zip, &extract_base, None).unwrap();

    // Verify it automatically created a container folder "loose_payload"
    assert_eq!(final_dir, extract_base.join("loose_payload"));
    assert!(final_dir.join("doc1.txt").exists());
    assert!(final_dir.join("doc2.txt").exists());
}

#[test]
fn test_tar_xz_lifecycle() {
    let (tmp, sources) = setup_test_files();
    let tarxz_path = tmp.path().join("test_lifecycle.tar.xz");
    let extract_dir = tmp.path().join("tarxz_out");

    let opts = AddOptions {
        compression_level: CompressionLevel::Normal,
        ..Default::default()
    };
    TarEngine::create(&tarxz_path, &sources, ArchiveType::TarXz, &opts).unwrap();
    assert!(tarxz_path.exists());

    // Verify magic bytes detection
    assert_eq!(detect_archive_type(&tarxz_path), ArchiveType::TarXz);

    // Test integrity
    assert!(TarEngine::test(&tarxz_path, ArchiveType::TarXz).unwrap());

    // Read entries
    let (info, entries) = TarEngine::read_entries(&tarxz_path, ArchiveType::TarXz).unwrap();
    assert_eq!(info.archive_type, ArchiveType::TarXz);
    assert!(entries.iter().any(|e| e.name == "hello.txt"));
    assert!(entries.iter().any(|e| e.name == "data.json"));

    // Extract
    TarEngine::extract(&tarxz_path, &extract_dir, ArchiveType::TarXz, None).unwrap();
    assert_eq!(
        fs::read_to_string(extract_dir.join("hello.txt")).unwrap(),
        "Hello from LinRAR Pure Rust!"
    );
    assert_eq!(
        fs::read_to_string(extract_dir.join("subfolder").join("nested.txt")).unwrap(),
        "Nested content for directory testing"
    );
}

#[test]
fn test_tar_zst_lifecycle() {
    let (tmp, sources) = setup_test_files();
    let tarzst_path = tmp.path().join("test_lifecycle.tar.zst");
    let extract_dir = tmp.path().join("tarzst_out");

    let opts = AddOptions {
        compression_level: CompressionLevel::Good,
        ..Default::default()
    };
    TarEngine::create(&tarzst_path, &sources, ArchiveType::TarZst, &opts).unwrap();
    assert!(tarzst_path.exists());

    // Verify magic bytes detection
    assert_eq!(detect_archive_type(&tarzst_path), ArchiveType::TarZst);

    // Test integrity
    assert!(TarEngine::test(&tarzst_path, ArchiveType::TarZst).unwrap());

    // Read entries
    let (info, entries) = TarEngine::read_entries(&tarzst_path, ArchiveType::TarZst).unwrap();
    assert_eq!(info.archive_type, ArchiveType::TarZst);
    assert!(entries.iter().any(|e| e.name == "hello.txt"));
    assert!(entries.iter().any(|e| e.name == "data.json"));

    // Extract
    TarEngine::extract(&tarzst_path, &extract_dir, ArchiveType::TarZst, None).unwrap();
    assert_eq!(
        fs::read_to_string(extract_dir.join("hello.txt")).unwrap(),
        "Hello from LinRAR Pure Rust!"
    );
    assert_eq!(
        fs::read_to_string(extract_dir.join("subfolder").join("nested.txt")).unwrap(),
        "Nested content for directory testing"
    );
}

#[test]
fn test_tar_modern_conversion() {
    let (tmp, sources) = setup_test_files();
    let zst_src = tmp.path().join("convert_source.tar.zst");
    TarEngine::create(
        &zst_src,
        &sources,
        ArchiveType::TarZst,
        &AddOptions::default(),
    )
    .unwrap();

    // Convert .tar.zst -> .tar.xz
    let converted_xz =
        ArchiveConverter::convert(&zst_src, ArchiveType::TarXz, &AddOptions::default()).unwrap();

    assert!(converted_xz.exists());
    assert_eq!(detect_archive_type(&converted_xz), ArchiveType::TarXz);

    // Extract converted .tar.xz to verify data preserved 100%
    let ext_dir = tmp.path().join("converted_xz_out");
    TarEngine::extract(&converted_xz, &ext_dir, ArchiveType::TarXz, None).unwrap();
    assert_eq!(
        fs::read_to_string(ext_dir.join("hello.txt")).unwrap(),
        "Hello from LinRAR Pure Rust!"
    );
}

#[test]
fn test_sevenzip_lifecycle() {
    let (tmp, sources) = setup_test_files();
    let sevenz_path = tmp.path().join("test_lifecycle.7z");
    let extract_dir = tmp.path().join("sevenz_out");

    let opts = AddOptions {
        compression_level: CompressionLevel::Normal,
        ..Default::default()
    };
    SevenZipEngine::create(&sevenz_path, &sources, &opts).unwrap();
    assert!(sevenz_path.exists());

    // Verify magic bytes detection
    assert_eq!(detect_archive_type(&sevenz_path), ArchiveType::SevenZip);

    // Test integrity
    assert!(SevenZipEngine::test(&sevenz_path, None).unwrap());

    // Read entries
    let (info, entries) = SevenZipEngine::read_entries(&sevenz_path, None).unwrap();
    assert_eq!(info.archive_type, ArchiveType::SevenZip);
    assert!(entries.iter().any(|e| e.name == "hello.txt"));
    assert!(entries.iter().any(|e| e.name == "data.json"));

    // Extract
    SevenZipEngine::extract(&sevenz_path, &extract_dir, None, None).unwrap();
    assert_eq!(
        fs::read_to_string(extract_dir.join("hello.txt")).unwrap(),
        "Hello from LinRAR Pure Rust!"
    );
    assert_eq!(
        fs::read_to_string(extract_dir.join("subfolder").join("nested.txt")).unwrap(),
        "Nested content for directory testing"
    );
}

#[test]
fn test_sevenzip_aes_encryption() {
    let (tmp, sources) = setup_test_files();
    let sevenz_path = tmp.path().join("encrypted.7z");
    let extract_dir = tmp.path().join("sevenz_enc_out");
    let pwd = "LinRAR_7z_SecretPassword";

    let opts = AddOptions {
        compression_level: CompressionLevel::Fast,
        password: Some(pwd.to_string()),
        encrypt_headers: true,
        ..Default::default()
    };
    SevenZipEngine::create(&sevenz_path, &sources, &opts).unwrap();
    assert!(sevenz_path.exists());

    // Read entries with password
    let (info, entries) = SevenZipEngine::read_entries(&sevenz_path, Some(pwd)).unwrap();
    assert_eq!(info.archive_type, ArchiveType::SevenZip);
    assert!(entries.iter().any(|e| e.encrypted));

    // Testing with wrong password should fail
    assert!(SevenZipEngine::test(&sevenz_path, Some("wrong_password")).is_err());

    // Testing with correct password should succeed
    assert!(SevenZipEngine::test(&sevenz_path, Some(pwd)).unwrap());

    // Extracting with wrong password should fail
    let bad_dir = tmp.path().join("bad_7z_out");
    assert!(SevenZipEngine::extract(&sevenz_path, &bad_dir, None, Some("wrong_password")).is_err());

    // Extracting with correct password should succeed and verify content
    SevenZipEngine::extract(&sevenz_path, &extract_dir, None, Some(pwd)).unwrap();
    assert_eq!(
        fs::read_to_string(extract_dir.join("hello.txt")).unwrap(),
        "Hello from LinRAR Pure Rust!"
    );
}

#[test]
fn test_sevenzip_conversion() {
    let (tmp, sources) = setup_test_files();
    let zip_src = tmp.path().join("convert_src_for_7z.zip");
    ZipEngine::create(&zip_src, &sources, &AddOptions::default()).unwrap();

    // Convert .zip -> .7z
    let converted_7z =
        ArchiveConverter::convert(&zip_src, ArchiveType::SevenZip, &AddOptions::default()).unwrap();

    assert!(converted_7z.exists());
    assert_eq!(detect_archive_type(&converted_7z), ArchiveType::SevenZip);

    // Extract converted .7z to verify data preserved 100%
    let ext_dir = tmp.path().join("converted_7z_out");
    SevenZipEngine::extract(&converted_7z, &ext_dir, None, None).unwrap();
    assert_eq!(
        fs::read_to_string(ext_dir.join("hello.txt")).unwrap(),
        "Hello from LinRAR Pure Rust!"
    );
}

#[test]
fn test_smart_bypass_extension_recognition() {
    assert!(SmartBypass::default_extensions().contains(&"mp4"));
    assert!(SmartBypass::default_extensions().contains(&"zip"));

    // Incompressible media & archives
    assert!(SmartBypass::is_incompressible_extension("mp4"));
    assert!(SmartBypass::is_incompressible_extension(".mkv"));
    assert!(SmartBypass::is_incompressible_extension("JPG"));
    assert!(SmartBypass::is_incompressible_extension(".Png"));
    assert!(SmartBypass::is_incompressible_extension("zip"));
    assert!(SmartBypass::is_incompressible_extension("7z"));
    assert!(SmartBypass::is_incompressible_extension("zst"));
    assert!(SmartBypass::is_incompressible_extension("pdf"));
    assert!(SmartBypass::is_incompressible_extension("docx"));

    // Compressible documents & code
    assert!(!SmartBypass::is_incompressible_extension("txt"));
    assert!(!SmartBypass::is_incompressible_extension("rs"));
    assert!(!SmartBypass::is_incompressible_extension("json"));
    assert!(!SmartBypass::is_incompressible_extension("csv"));
    assert!(!SmartBypass::is_incompressible_extension("log"));
}

#[test]
fn test_smart_bypass_entropy_calculation() {
    // 1. Low entropy (repetitive data)
    let repetitive = vec![b'A'; 8192];
    let low_entropy = SmartBypass::calculate_entropy(&repetitive);
    assert!(low_entropy < 0.1);

    // 2. High entropy (random / encrypted simulation)
    // Create byte distribution covering all 256 byte values uniformly
    let mut high_entropy_data = Vec::with_capacity(8192);
    for i in 0..8192 {
        high_entropy_data.push((i % 256) as u8);
    }
    let high_entropy = SmartBypass::calculate_entropy(&high_entropy_data);
    assert!(high_entropy >= 7.9);

    // 3. File heuristic test
    let tmp = tempfile::tempdir().unwrap();
    let random_file = tmp.path().join("data.unknown_ext");
    fs::write(&random_file, &high_entropy_data).unwrap();
    assert!(SmartBypass::should_bypass(&random_file));

    let text_file = tmp.path().join("data.txt");
    fs::write(
        &text_file,
        "This is plain compressible English text repeat. ".repeat(100),
    )
    .unwrap();
    assert!(!SmartBypass::should_bypass(&text_file));
}

#[test]
fn test_smart_bypass_zip_compression_behavior() {
    let tmp = tempfile::tempdir().unwrap();

    // Create compressible file
    let text_path = tmp.path().join("compressible.txt");
    fs::write(
        &text_path,
        "Repeatable text data to be deflated. ".repeat(200),
    )
    .unwrap();

    // Create incompressible file (.mp4 extension)
    let video_path = tmp.path().join("recording.mp4");
    let mut pseudo_video = Vec::with_capacity(4096);
    for i in 0..4096 {
        pseudo_video.push(((i * 37 + 13) % 256) as u8);
    }
    fs::write(&video_path, &pseudo_video).unwrap();

    let sources = vec![text_path.clone(), video_path.clone()];

    // Case 1: Smart bypass enabled (default)
    let zip_smart = tmp.path().join("smart_test.zip");
    let smart_opts = AddOptions {
        compression_level: CompressionLevel::Normal,
        smart_bypass: true,
        ..Default::default()
    };
    ZipEngine::create(&zip_smart, &sources, &smart_opts).unwrap();

    let (_, entries_smart) = ZipEngine::read_entries(&zip_smart).unwrap();
    let text_entry = entries_smart
        .iter()
        .find(|e| e.name == "compressible.txt")
        .unwrap();
    let video_entry = entries_smart
        .iter()
        .find(|e| e.name == "recording.mp4")
        .unwrap();

    // Text file should be compressed (packed < unpacked)
    assert!(text_entry.packed_size < text_entry.size);
    // Video file with smart bypass should be Stored (packed == unpacked)
    assert_eq!(video_entry.packed_size, video_entry.size);

    // Case 2: Smart bypass disabled (--no-smart-bypass)
    let zip_no_smart = tmp.path().join("no_smart_test.zip");
    let no_smart_opts = AddOptions {
        compression_level: CompressionLevel::Normal,
        smart_bypass: false,
        ..Default::default()
    };
    ZipEngine::create(&zip_no_smart, &sources, &no_smart_opts).unwrap();

    let file = fs::File::open(&zip_no_smart).unwrap();
    let mut arch = zip::ZipArchive::new(file).unwrap();
    let vid_file = arch.by_name("recording.mp4").unwrap();
    // With smart bypass disabled, the compression method must be Deflated
    assert_eq!(vid_file.compression(), zip::CompressionMethod::Deflated);
}

#[test]
fn test_shred_file_quick_zero() {
    let tmp = tempfile::tempdir().unwrap();
    let file_path = tmp.path().join("secret_data.txt");
    let content = "Highly confidential financial statement 2026";
    fs::write(&file_path, content).unwrap();
    assert!(file_path.exists());

    let bytes = FileShredder::shred_file(&file_path, ShredMethod::QuickZero).unwrap();
    assert_eq!(bytes, content.len() as u64);
    assert!(!file_path.exists());
}

#[test]
fn test_shred_file_dod5220() {
    let tmp = tempfile::tempdir().unwrap();
    let file_path = tmp.path().join("dod_secret.bin");
    let content = vec![0x42u8; 1024];
    fs::write(&file_path, &content).unwrap();
    assert!(file_path.exists());

    let bytes = FileShredder::shred_file(&file_path, ShredMethod::DoD5220).unwrap();
    // 3 passes overwrite
    assert_eq!(bytes, (content.len() * 3) as u64);
    assert!(!file_path.exists());
}

#[test]
fn test_shred_path_recursive_directory() {
    let tmp = tempfile::tempdir().unwrap();
    let target_dir = tmp.path().join("classified_folder");
    fs::create_dir_all(target_dir.join("sub1").join("sub2")).unwrap();

    let f1 = target_dir.join("root.txt");
    let f2 = target_dir.join("sub1").join("doc1.txt");
    let f3 = target_dir.join("sub1").join("sub2").join("deep.txt");

    fs::write(&f1, "Sensitive 1").unwrap();
    fs::write(&f2, "Sensitive 2").unwrap();
    fs::write(&f3, "Sensitive 3").unwrap();

    let report = FileShredder::shred_path(&target_dir, ShredMethod::Random).unwrap();
    assert_eq!(report.files_shredded, 3);
    assert_eq!(report.dirs_removed, 3); // target_dir, sub1, sub2
    assert!(report.bytes_overwritten > 0);
    assert!(!target_dir.exists());
}

#[test]
fn test_archive_creation_with_shred() {
    let tmp = tempfile::tempdir().unwrap();
    let file_to_archive = tmp.path().join("archive_me_then_shred.txt");
    fs::write(
        &file_to_archive,
        "Document to be safely shredded after archive",
    )
    .unwrap();
    assert!(file_to_archive.exists());

    let zip_dest = tmp.path().join("shredded_source.zip");
    let sources = vec![file_to_archive.clone()];
    let opts = AddOptions {
        delete_after_archiving: true,
        shred_after_archiving: true,
        shred_method: ShredMethod::DoD5220,
        ..Default::default()
    };

    ZipEngine::create(&zip_dest, &sources, &opts).unwrap();
    assert!(zip_dest.exists());

    // Execute post-archiving shredding
    if opts.delete_after_archiving && opts.shred_after_archiving {
        FileShredder::shred_paths(&sources, opts.shred_method);
    }

    // Verify source file is completely deleted and sanitized
    assert!(!file_to_archive.exists());

    // Verify archive still unpacks the preserved data properly
    let unpack_dir = tmp.path().join("unpacked_shred_test");
    ZipEngine::extract(&zip_dest, &unpack_dir, None, None).unwrap();
    assert_eq!(
        fs::read_to_string(unpack_dir.join("archive_me_then_shred.txt")).unwrap(),
        "Document to be safely shredded after archive"
    );
}

fn synthesize_test_iso(path: &std::path::Path) {
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
    let vol_id = b"TEST_VOLUME";
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
    pvd[156] = 34; // record length
    pvd[157] = 0;
    pvd[158..162].copy_from_slice(&18u32.to_le_bytes()); // Root LBA = 18
    pvd[162..166].copy_from_slice(&18u32.to_be_bytes());
    pvd[166..170].copy_from_slice(&2048u32.to_le_bytes()); // Root Len = 2048
    pvd[170..174].copy_from_slice(&2048u32.to_be_bytes());
    pvd[174..181].copy_from_slice(&[126, 10, 19, 7, 0, 0, 0]); // 2026-10-19
    pvd[181] = 0x02; // Directory flag
    pvd[188] = 1; // len_fi
    pvd[189] = 0; // self

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
        if rec_len % 2 != 0 {
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
        buf[pos + 28..pos + 30].copy_from_slice(&1u16.to_le_bytes());
        buf[pos + 30..pos + 32].copy_from_slice(&1u16.to_be_bytes());
        buf[pos + 32] = name_len as u8;
        buf[pos + 33..pos + 33 + name_len].copy_from_slice(name);
        rec_len
    }

    // Sector 18: Root Directory Records
    let root = &mut data[18 * sector_size..19 * sector_size];
    let mut pos = 0;
    pos += write_record(root, pos, 18, 2048, true, &[0]); // .
    pos += write_record(root, pos, 18, 2048, true, &[1]); // ..
    let file1_content = b"Hello ISO 9660 from LinRAR!";
    pos += write_record(
        root,
        pos,
        19,
        file1_content.len() as u32,
        false,
        b"HELLO.TXT;1",
    );
    let _ = write_record(root, pos, 20, 2048, true, b"SUBDIR");

    // Sector 19: File 1 content
    let s19 = &mut data[19 * sector_size..20 * sector_size];
    s19[..file1_content.len()].copy_from_slice(file1_content);

    // Sector 20: Subdirectory records
    let sub = &mut data[20 * sector_size..21 * sector_size];
    let mut spos = 0;
    spos += write_record(sub, spos, 20, 2048, true, &[0]); // .
    spos += write_record(sub, spos, 18, 2048, true, &[1]); // ..
    let nested_content = b"Nested file in ISO container";
    let _ = write_record(
        sub,
        spos,
        21,
        nested_content.len() as u32,
        false,
        b"NESTED.DAT;1",
    );

    // Sector 21: Nested file content
    let s21 = &mut data[21 * sector_size..22 * sector_size];
    s21[..nested_content.len()].copy_from_slice(nested_content);

    fs::write(path, data).unwrap();
}

#[test]
fn test_iso_detection_and_listing() {
    let tmp = tempfile::tempdir().unwrap();
    let iso_path = tmp.path().join("test_image.iso");
    synthesize_test_iso(&iso_path);

    // 1. Verify archive detector recognizes ISO
    assert_eq!(detect_archive_type(&iso_path), ArchiveType::Iso);

    // 2. Read entries
    let (info, entries) = IsoEngine::read_entries(&iso_path).unwrap();
    assert_eq!(info.archive_type, ArchiveType::Iso);
    assert!(info.comment.as_ref().unwrap().contains("TEST_VOLUME"));
    assert_eq!(info.total_files, 2);
    assert_eq!(info.total_folders, 1);

    // Verify entries
    let hello_entry = entries.iter().find(|e| e.name == "HELLO.TXT");
    assert!(hello_entry.is_some());
    let hello = hello_entry.unwrap();
    assert_eq!(hello.full_path, "HELLO.TXT");
    assert_eq!(hello.size, 27);
    assert!(!hello.is_dir);

    let subdir_entry = entries.iter().find(|e| e.name == "SUBDIR");
    assert!(subdir_entry.is_some());
    assert!(subdir_entry.unwrap().is_dir);

    let nested_entry = entries.iter().find(|e| e.name == "NESTED.DAT");
    assert!(nested_entry.is_some());
    let nested = nested_entry.unwrap();
    assert_eq!(nested.full_path, "SUBDIR/NESTED.DAT");
    assert_eq!(nested.size, 28);
    assert!(!nested.is_dir);
}

#[test]
fn test_iso_integrity_verification() {
    let tmp = tempfile::tempdir().unwrap();
    let iso_path = tmp.path().join("test_verify.iso");
    synthesize_test_iso(&iso_path);

    // Intact ISO passes
    assert!(IsoEngine::test(&iso_path).unwrap());

    // Truncated ISO fails
    let truncated_path = tmp.path().join("truncated.iso");
    let mut data = fs::read(&iso_path).unwrap();
    data.truncate(19 * 2048 + 10); // Cuts off sector 20 & 21
    fs::write(&truncated_path, data).unwrap();
    assert!(IsoEngine::test(&truncated_path).is_err());
}

#[test]
fn test_iso_extraction_all_and_selective() {
    let tmp = tempfile::tempdir().unwrap();
    let iso_path = tmp.path().join("test_extract.iso");
    synthesize_test_iso(&iso_path);

    // 1. Full extraction
    let extract_dir = tmp.path().join("extracted_full");
    IsoEngine::extract(&iso_path, &extract_dir, None).unwrap();

    let hello_content = fs::read_to_string(extract_dir.join("HELLO.TXT")).unwrap();
    assert_eq!(hello_content, "Hello ISO 9660 from LinRAR!");

    let nested_content = fs::read_to_string(extract_dir.join("SUBDIR").join("NESTED.DAT")).unwrap();
    assert_eq!(nested_content, "Nested file in ISO container");

    // 2. Selective extraction
    let selective_dir = tmp.path().join("extracted_selective");
    IsoEngine::extract(&iso_path, &selective_dir, Some(&["HELLO.TXT".to_string()])).unwrap();

    assert!(selective_dir.join("HELLO.TXT").exists());
    assert!(!selective_dir.join("SUBDIR").join("NESTED.DAT").exists());
}

#[test]
fn test_iso_conversion_to_zip() {
    let tmp = tempfile::tempdir().unwrap();
    let iso_path = tmp.path().join("disk_image.iso");
    synthesize_test_iso(&iso_path);

    let opts = AddOptions::default();
    let zip_dest = ArchiveConverter::convert(&iso_path, ArchiveType::Zip, &opts).unwrap();
    assert!(zip_dest.exists());
    assert_eq!(detect_archive_type(&zip_dest), ArchiveType::Zip);

    // Extract the converted zip to verify integrity
    let out_dir = tmp.path().join("zip_out");
    ZipEngine::extract(&zip_dest, &out_dir, None, None).unwrap();

    assert_eq!(
        fs::read_to_string(out_dir.join("HELLO.TXT")).unwrap(),
        "Hello ISO 9660 from LinRAR!"
    );
    assert_eq!(
        fs::read_to_string(out_dir.join("SUBDIR").join("NESTED.DAT")).unwrap(),
        "Nested file in ISO container"
    );
}

#[test]
fn test_iso_smart_extraction() {
    let tmp = tempfile::tempdir().unwrap();
    let iso_path = tmp.path().join("loose_disk.iso");
    synthesize_test_iso(&iso_path);

    let out_base = tmp.path().join("smart_out");
    // Since our ISO has both HELLO.TXT and SUBDIR at root (multiple items), smart extract creates a folder named after the ISO stem
    let final_dest = SmartExtractor::extract_smart(&iso_path, &out_base, None).unwrap();
    assert_eq!(final_dest, out_base.join("loose_disk"));
    assert!(final_dest.join("HELLO.TXT").exists());
    assert!(final_dest.join("SUBDIR").join("NESTED.DAT").exists());
}
