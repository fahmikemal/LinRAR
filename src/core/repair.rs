use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use crate::core::detector::detect_archive_type;
use crate::core::rar_engine::RarEngine;
use crate::types::ArchiveType;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct RepairResult {
    pub success: bool,
    pub original_path: PathBuf,
    pub fixed_path: PathBuf,
    pub log: String,
}

pub struct ArchiveRepair;

impl ArchiveRepair {
    pub fn repair<P: AsRef<Path>>(archive_path: P) -> Result<RepairResult, String> {
        let path = archive_path.as_ref();
        if !path.exists() {
            return Err("Archive file does not exist".to_string());
        }

        let arch_type = detect_archive_type(path);
        match arch_type {
            ArchiveType::Rar5 | ArchiveType::Rar4 => Self::repair_rar(path),
            ArchiveType::Zip => Self::repair_zip(path),
            _ => Err("Repair is currently supported for RAR and ZIP archives".to_string()),
        }
    }

    fn repair_rar(path: &Path) -> Result<RepairResult, String> {
        let bin = RarEngine::get_rar_bin().ok_or("RAR binary not found")?;
        let parent = path.parent().unwrap_or(Path::new("."));
        let file_name = path.file_name().unwrap_or_default().to_string_lossy();
        let fixed_name = format!("fixed.{}", file_name);
        let fixed_path = parent.join(&fixed_name);

        let mut cmd = Command::new(bin);
        cmd.arg("r")
            .arg("-y")
            .arg("-p-")
            .arg(path)
            .current_dir(parent)
            .stdin(Stdio::null());

        let output = cmd.output().map_err(|e| e.to_string())?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        let log = if stderr.trim().is_empty() { stdout } else { format!("{}\n{}", stdout, stderr) };
        let success = fixed_path.exists() || log.contains("done") || log.contains("Done");

        Ok(RepairResult {
            success,
            original_path: path.to_path_buf(),
            fixed_path,
            log,
        })
    }

    fn repair_zip(path: &Path) -> Result<RepairResult, String> {
        // Try zip -F or zip -FF if zip command exists
        let parent = path.parent().unwrap_or(Path::new("."));
        let file_name = path.file_name().unwrap_or_default().to_string_lossy();
        let fixed_name = format!("fixed.{}", file_name);
        let fixed_path = parent.join(&fixed_name);

        let mut log = String::new();
        let mut success = false;

        if Command::new("which").arg("zip").output().map(|o| o.status.success()).unwrap_or(false) {
            let output = Command::new("zip")
                .arg("-FF")
                .arg(path)
                .arg("--out")
                .arg(&fixed_path)
                .stdin(Stdio::null())
                .output();

            if let Ok(out) = output {
                log = String::from_utf8_lossy(&out.stdout).to_string();
                success = fixed_path.exists();
            }
        }

        if !success {
            // Fallback: Use RarEngine to repair ZIP (RAR binary has built-in ZIP repair mode)
            if let Some(bin) = RarEngine::get_rar_bin() {
                let output = Command::new(bin)
                    .arg("r")
                    .arg("-y")
                    .arg("-p-")
                    .arg(path)
                    .current_dir(parent)
                    .stdin(Stdio::null())
                    .output();

                if let Ok(out) = output {
                    log = String::from_utf8_lossy(&out.stdout).to_string();
                    success = fixed_path.exists() || log.contains("done") || log.contains("Done");
                }
            }
        }

        Ok(RepairResult {
            success,
            original_path: path.to_path_buf(),
            fixed_path,
            log,
        })
    }
}
