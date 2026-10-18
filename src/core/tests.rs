use std::fs;
use std::path::PathBuf;

use crate::core::benchmark::BenchmarkRunner;
use crate::core::converter::ArchiveConverter;
use crate::core::detector::detect_archive_type;
use crate::core::finder::ArchiveFinder;
use crate::core::rar_engine::RarEngine;
use crate::core::report::{ArchiveReport, ReportFormat};
use crate::core::scanner::VirusScanner;
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

    let opts = AddOptions::default();
    ZipEngine::create(&zip_path, &sources, &opts).unwrap();
    TarEngine::create(&targz_path, &sources, ArchiveType::TarGz, &opts).unwrap();
    TarEngine::create(&tarxz_path, &sources, ArchiveType::TarXz, &opts).unwrap();
    TarEngine::create(&tarzst_path, &sources, ArchiveType::TarZst, &opts).unwrap();

    assert_eq!(detect_archive_type(&zip_path), ArchiveType::Zip);
    assert_eq!(detect_archive_type(&targz_path), ArchiveType::TarGz);
    assert_eq!(detect_archive_type(&tarxz_path), ArchiveType::TarXz);
    assert_eq!(detect_archive_type(&tarzst_path), ArchiveType::TarZst);
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
