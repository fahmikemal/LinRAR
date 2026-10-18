use std::fs;
use std::path::PathBuf;

use linrar::core::benchmark::BenchmarkRunner;
use linrar::core::detector::detect_archive_type;
use linrar::core::rar_engine::RarEngine;
use linrar::core::tar_engine::TarEngine;
use linrar::core::zip_engine::ZipEngine;
use linrar::types::{AddOptions, ArchiveType, CompressionLevel};

fn setup_test_files() -> (tempfile::TempDir, Vec<PathBuf>) {
    let tmp = tempfile::tempdir().unwrap();
    let file1 = tmp.path().join("hello.txt");
    let file2 = tmp.path().join("data.json");
    let subfolder = tmp.path().join("subfolder");
    fs::create_dir_all(&subfolder).unwrap();
    let file3 = subfolder.join("nested.txt");

    fs::write(&file1, "Hello from LinRAR Pure Rust!").unwrap();
    fs::write(&file2, r#"{"app": "LinRAR", "version": "0.1.0", "language": "Rust"}"#).unwrap();
    fs::write(&file3, "Nested content for directory testing").unwrap();

    let sources = vec![file1, file2, subfolder];
    (tmp, sources)
}

#[test]
fn test_magic_detection() {
    let (tmp, sources) = setup_test_files();
    let zip_path = tmp.path().join("test.zip");
    let targz_path = tmp.path().join("test.tar.gz");
    let rar_path = tmp.path().join("test.rar");

    let opts = AddOptions::default();
    ZipEngine::create(&zip_path, &sources, &opts).unwrap();
    TarEngine::create(&targz_path, &sources, ArchiveType::TarGz, &opts).unwrap();
    RarEngine::create(&rar_path, &sources, &opts).unwrap();

    assert_eq!(detect_archive_type(&zip_path), ArchiveType::Zip);
    assert_eq!(detect_archive_type(&targz_path), ArchiveType::TarGz);
    assert_eq!(detect_archive_type(&rar_path), ArchiveType::Rar5);
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
    ZipEngine::extract(&zip_path, &extract_dir, None).unwrap();
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

    let clean_res = linrar::core::scanner::VirusScanner::scan_archive(&clean_zip).unwrap();
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

    let infected_res = linrar::core::scanner::VirusScanner::scan_archive(&infected_zip).unwrap();
    assert!(!infected_res.clean);
    assert!(infected_res.threats.iter().any(|t| t.threat_name.contains("EICAR")));
}

#[test]
fn test_archive_report() {
    let (tmp, sources) = setup_test_files();
    let zip_path = tmp.path().join("report_test.zip");
    ZipEngine::create(&zip_path, &sources, &AddOptions::default()).unwrap();

    let txt_report = linrar::core::report::ArchiveReport::generate(&zip_path, linrar::core::report::ReportFormat::Text).unwrap();
    assert!(txt_report.contains("LinRAR 0.1.0 - Archive Contents Report"));
    assert!(txt_report.contains("hello.txt"));
    assert!(txt_report.contains("data.json"));

    let html_report = linrar::core::report::ArchiveReport::generate(&zip_path, linrar::core::report::ReportFormat::Html).unwrap();
    assert!(html_report.contains("<!DOCTYPE html>"));
    assert!(html_report.contains("LinRAR 0.1.0 - Archive Contents Report"));
    assert!(html_report.contains("hello.txt"));
}

#[test]
fn test_archive_finder() {
    let (tmp, sources) = setup_test_files();
    let zip_path = tmp.path().join("search_container.zip");
    ZipEngine::create(&zip_path, &sources, &AddOptions::default()).unwrap();

    let matches = linrar::core::finder::ArchiveFinder::find_in_directory(tmp.path(), "hello", true);
    assert!(!matches.is_empty());
    assert!(matches.iter().any(|m| m.file_name == "hello.txt"));
}

#[test]
fn test_archive_converter() {
    let (tmp, sources) = setup_test_files();
    let zip_path = tmp.path().join("convert_src.zip");
    ZipEngine::create(&zip_path, &sources, &AddOptions::default()).unwrap();

    let target = linrar::core::converter::ArchiveConverter::convert(
        &zip_path,
        ArchiveType::TarGz,
        &AddOptions::default(),
    ).unwrap();

    assert!(target.exists());
    assert_eq!(detect_archive_type(&target), ArchiveType::TarGz);
}

#[test]
fn test_rar_advanced_operations() {
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
    TarEngine::create(&targz_path, &sources, ArchiveType::TarGz, &AddOptions::default()).unwrap();

    assert!(ZipEngine::test(&zip_path).unwrap());
    assert!(TarEngine::test(&targz_path, ArchiveType::TarGz).unwrap());
}

