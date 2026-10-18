use std::fs::{self, File};
use std::io::Read;
use std::path::Path;
use std::process::Command;
use walkdir::WalkDir;

use crate::core::detector::detect_archive_type;
use crate::core::rar_engine::RarEngine;
use crate::core::tar_engine::TarEngine;
use crate::core::zip_engine::ZipEngine;
use crate::types::ArchiveType;

const EICAR_TEST_STRING: &[u8] =
    b"X5O!P%@AP[4\\PZX54(P^)7CC)7}$EICAR-STANDARD-ANTIVIRUS-TEST-FILE!$H+H*";

#[derive(Debug, Clone)]
pub struct ThreatItem {
    pub file: String,
    pub threat_name: String,
    pub severity: String,
}

#[derive(Debug, Clone)]
pub struct ScanResult {
    pub archive_name: String,
    pub files_scanned: usize,
    pub scanner_name: String,
    pub threats: Vec<ThreatItem>,
    pub clean: bool,
}

pub struct VirusScanner;

impl VirusScanner {
    pub fn scan_archive<P: AsRef<Path>>(archive_path: P) -> Result<ScanResult, String> {
        let arch_path = archive_path.as_ref();
        if !arch_path.exists() {
            return Err("Archive file does not exist".to_string());
        }

        let arch_type = detect_archive_type(arch_path);
        let temp_dir = std::env::temp_dir().join(format!("linrar_scan_{}", std::process::id()));
        fs::create_dir_all(&temp_dir).map_err(|e| e.to_string())?;

        // Extract archive to temp directory for inspection
        let extract_res = match arch_type {
            ArchiveType::Zip => ZipEngine::extract(arch_path, &temp_dir, None),
            ArchiveType::TarGz => {
                TarEngine::extract(arch_path, &temp_dir, ArchiveType::TarGz, None)
            }
            ArchiveType::TarBz2 => {
                TarEngine::extract(arch_path, &temp_dir, ArchiveType::TarBz2, None)
            }
            ArchiveType::Tar => TarEngine::extract(arch_path, &temp_dir, ArchiveType::Tar, None),
            _ => RarEngine::extract(arch_path, &temp_dir, None, None),
        };

        if let Err(e) = extract_res {
            let _ = fs::remove_dir_all(&temp_dir);
            return Err(format!("Failed to unpack archive for scanning: {}", e));
        }

        // Try ClamAV if available on Linux system
        let scan_result = if Self::has_clamscan() {
            Self::scan_with_clamscan(&temp_dir, arch_path)
        } else {
            Self::scan_heuristic(&temp_dir, arch_path)
        };

        // Clean up temporary extraction
        let _ = fs::remove_dir_all(&temp_dir);

        Ok(scan_result)
    }

    fn has_clamscan() -> bool {
        Command::new("which")
            .arg("clamscan")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn scan_with_clamscan(temp_dir: &Path, arch_path: &Path) -> ScanResult {
        let output = Command::new("clamscan")
            .arg("-r")
            .arg("--no-summary")
            .arg(temp_dir)
            .output();

        let mut threats = Vec::new();
        let mut files_scanned = 0;

        if let Ok(out) = output {
            let stdout = String::from_utf8_lossy(&out.stdout);
            for line in stdout.lines() {
                if line.contains(": OK") {
                    files_scanned += 1;
                } else if line.contains("FOUND") {
                    files_scanned += 1;
                    let parts: Vec<&str> = line.split(':').collect();
                    let file_raw = parts.first().unwrap_or(&"unknown");
                    let file_rel = file_raw
                        .strip_prefix(temp_dir.to_str().unwrap_or(""))
                        .unwrap_or(file_raw);
                    let threat_name = parts
                        .get(1)
                        .unwrap_or(&"Infected")
                        .replace("FOUND", "")
                        .trim()
                        .to_string();

                    threats.push(ThreatItem {
                        file: file_rel.trim_start_matches('/').to_string(),
                        threat_name,
                        severity: "High".to_string(),
                    });
                }
            }
        }

        ScanResult {
            archive_name: arch_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            files_scanned,
            scanner_name: "ClamAV Antivirus Scanner".to_string(),
            clean: threats.is_empty(),
            threats,
        }
    }

    fn scan_heuristic(temp_dir: &Path, arch_path: &Path) -> ScanResult {
        let mut files_scanned = 0;
        let mut threats = Vec::new();

        for entry in WalkDir::new(temp_dir).into_iter().filter_map(|e| e.ok()) {
            if entry.file_type().is_file() {
                files_scanned += 1;
                let path = entry.path();
                let rel_path = path
                    .strip_prefix(temp_dir)
                    .unwrap_or(path)
                    .to_string_lossy()
                    .to_string();
                let filename_lower = rel_path.to_lowercase();

                // Check 1: EICAR Standard Antivirus Test Pattern
                if let Ok(mut f) = File::open(path) {
                    let mut buf = vec![0u8; 1024];
                    if let Ok(n) = f.read(&mut buf) {
                        if buf[..n]
                            .windows(EICAR_TEST_STRING.len())
                            .any(|w| w == EICAR_TEST_STRING)
                        {
                            threats.push(ThreatItem {
                                file: rel_path.clone(),
                                threat_name: "EICAR-Test-Signature (Standard AV Test Pattern)"
                                    .to_string(),
                                severity: "Critical".to_string(),
                            });
                            continue;
                        }

                        // Check 2: Suspicious embedded Windows PE in non-executable extension
                        if n >= 2
                            && &buf[..2] == b"MZ"
                            && (filename_lower.ends_with(".jpg")
                                || filename_lower.ends_with(".png")
                                || filename_lower.ends_with(".pdf")
                                || filename_lower.ends_with(".mp4"))
                        {
                            threats.push(ThreatItem {
                                file: rel_path.clone(),
                                threat_name: "Disguised-Executable (MZ Header in Media/Doc File)"
                                    .to_string(),
                                severity: "High".to_string(),
                            });
                            continue;
                        }

                        // Check 3: Double extension deception trick (e.g. invoice.pdf.exe)
                        if filename_lower.contains(".pdf.exe")
                            || filename_lower.contains(".doc.exe")
                            || filename_lower.contains(".txt.sh")
                            || filename_lower.contains(".jpg.scr")
                        {
                            threats.push(ThreatItem {
                                file: rel_path.clone(),
                                threat_name: "Suspicious-Double-Extension (Deceptive File Name)"
                                    .to_string(),
                                severity: "Warning".to_string(),
                            });
                        }
                    }
                }
            }
        }

        ScanResult {
            archive_name: arch_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            files_scanned,
            scanner_name: "LinRAR Heuristic Threat Engine".to_string(),
            clean: threats.is_empty(),
            threats,
        }
    }
}
