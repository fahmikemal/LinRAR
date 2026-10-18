use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::core::rar_parser::RarParser;
use crate::types::{AddOptions, ArchiveEntry, ArchiveInfo, ArchiveType};

pub struct RarEngine;

impl RarEngine {
    pub fn get_rar_bin() -> Option<PathBuf> {
        let mut candidates = vec![
            PathBuf::from("linrar_app/bin/rar"),
            PathBuf::from("/usr/local/bin/rar"),
            PathBuf::from("/usr/bin/rar"),
        ];

        if let Ok(exe) = std::env::current_exe() {
            if let Some(parent) = exe.parent() {
                candidates.push(parent.join("linrar_app/bin/rar"));
                candidates.push(parent.join("bin/rar"));
            }
        }

        for p in candidates {
            if p.exists() && p.is_file() {
                return Some(p);
            }
        }

        if let Ok(output) = Command::new("which").arg("rar").output() {
            if output.status.success() {
                let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !s.is_empty() {
                    return Some(PathBuf::from(s));
                }
            }
        }

        None
    }

    pub fn get_unrar_bin() -> Option<PathBuf> {
        let mut candidates = vec![
            PathBuf::from("linrar_app/bin/unrar"),
            PathBuf::from("/usr/local/bin/unrar"),
            PathBuf::from("/usr/bin/unrar"),
        ];

        if let Ok(exe) = std::env::current_exe() {
            if let Some(parent) = exe.parent() {
                candidates.push(parent.join("linrar_app/bin/unrar"));
                candidates.push(parent.join("bin/unrar"));
            }
        }

        for p in candidates {
            if p.exists() && p.is_file() {
                return Some(p);
            }
        }

        if let Ok(output) = Command::new("which").arg("unrar").output() {
            if output.status.success() {
                let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !s.is_empty() {
                    return Some(PathBuf::from(s));
                }
            }
        }

        Self::get_rar_bin()
    }

    pub fn read_entries<P: AsRef<Path>>(path: P) -> Result<(ArchiveInfo, Vec<ArchiveEntry>), String> {
        // Try pure Rust parser first
        if let Ok((info, entries)) = RarParser::parse(&path) {
            if !entries.is_empty() {
                return Ok((info, entries));
            }
        }

        // Fallback to CLI listing if pure parser found empty (e.g. encrypted headers)
        Self::list_via_cli(path)
    }

    fn list_via_cli<P: AsRef<Path>>(path: P) -> Result<(ArchiveInfo, Vec<ArchiveEntry>), String> {
        let bin = Self::get_unrar_bin().ok_or("UnRAR binary not found")?;
        let output = Command::new(bin)
            .arg("v")
            .arg("-c-")
            .arg("-y")
            .arg("-p-")
            .arg(path.as_ref())
            .stdin(Stdio::null())
            .output()
            .map_err(|e| e.to_string())?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut entries = Vec::new();
        let mut total_unpacked = 0u64;
        let mut total_packed = 0u64;
        let mut in_table = false;
        let is_solid = stdout.contains("Solid archive");
        let is_locked = stdout.contains("Lock is present") || stdout.contains("Locked archive");

        for line in stdout.lines() {
            if line.contains("-------------------------------------------------------------------------------") {
                in_table = !in_table;
                continue;
            }

            if in_table {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 6 {
                    let full_name = parts[0].to_string();
                    let name = full_name.split('/').last().unwrap_or(&full_name).to_string();
                    let size = parts[1].parse::<u64>().unwrap_or(0);
                    let packed_size = parts[2].parse::<u64>().unwrap_or(0);
                    let is_dir = line.contains(" <DIR> ") || full_name.ends_with('/');
                    let crc = parts.last().unwrap_or(&"").to_string();

                    total_unpacked += size;
                    total_packed += packed_size;

                    entries.push(ArchiveEntry {
                        name,
                        full_path: full_name,
                        size,
                        packed_size,
                        modified: "-".to_string(),
                        crc,
                        is_dir,
                        encrypted: false,
                        attributes: "-".to_string(),
                    });
                }
            }
        }

        let total_files = entries.iter().filter(|e| !e.is_dir).count();
        let total_folders = entries.iter().filter(|e| e.is_dir).count();

        Ok((
            ArchiveInfo {
                path: path.as_ref().to_path_buf(),
                archive_type: ArchiveType::Rar5,
                total_files,
                total_folders,
                unpacked_size: total_unpacked,
                packed_size: total_packed,
                is_solid,
                is_locked,
                has_comment: false,
                comment: None,
            },
            entries,
        ))
    }

    pub fn extract<P: AsRef<Path>, D: AsRef<Path>>(
        archive_path: P,
        dest_dir: D,
        selected_files: Option<&[String]>,
        password: Option<&str>,
    ) -> Result<(), String> {
        let bin = Self::get_unrar_bin().ok_or("UnRAR binary not found")?;
        let dest = dest_dir.as_ref();
        fs::create_dir_all(dest).map_err(|e| e.to_string())?;

        let mut cmd = Command::new(bin);
        cmd.arg("x")
            .arg("-y")
            .arg(format!("-op{}", dest.to_string_lossy()));

        if let Some(pwd) = password {
            cmd.arg(format!("-p{}", pwd));
        } else {
            cmd.arg("-p-");
        }

        cmd.arg(archive_path.as_ref());

        if let Some(files) = selected_files {
            for f in files {
                cmd.arg(f);
            }
        }

        cmd.stdin(Stdio::null());
        let output = cmd.output().map_err(|e| e.to_string())?;
        if output.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }

    pub fn create<P: AsRef<Path>>(
        archive_path: P,
        source_paths: &[PathBuf],
        options: &AddOptions,
    ) -> Result<(), String> {
        let bin = Self::get_rar_bin().ok_or("RAR binary not found")?;
        let mut cmd = Command::new(bin);

        cmd.arg("a").arg("-y").arg("-ep1");
        cmd.arg(format!("-m{}", options.compression_level as u8));

        if options.is_solid {
            cmd.arg("-s");
        }

        if options.lock_archive {
            cmd.arg("-k");
        }

        if let Some(vol) = &options.volume_size {
            cmd.arg(format!("-v{}", vol));
        }

        if let Some(pwd) = &options.password {
            if options.encrypt_headers {
                cmd.arg(format!("-hp{}", pwd));
            } else {
                cmd.arg(format!("-p{}", pwd));
            }
        } else {
            cmd.arg("-p-");
        }

        let mut temp_comment_path = None;
        if let Some(cmt) = &options.comment {
            let tmp = std::env::temp_dir().join(format!("linrar_cmt_{}.txt", std::process::id()));
            if let Ok(mut f) = File::create(&tmp) {
                let _ = f.write_all(cmt.as_bytes());
                cmd.arg(format!("-z{}", tmp.to_string_lossy()));
                temp_comment_path = Some(tmp);
            }
        }

        cmd.arg(archive_path.as_ref());

        for src in source_paths {
            cmd.arg(src);
        }

        cmd.stdin(Stdio::null());
        let output = cmd.output().map_err(|e| e.to_string())?;

        if let Some(tmp) = temp_comment_path {
            let _ = fs::remove_file(tmp);
        }

        if output.status.success() {
            Ok(())
        } else {
            let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
            if err.is_empty() {
                Err(String::from_utf8_lossy(&output.stdout).trim().to_string())
            } else {
                Err(err)
            }
        }
    }

    pub fn test<P: AsRef<Path>>(archive_path: P) -> Result<bool, String> {
        let bin = Self::get_unrar_bin().ok_or("UnRAR binary not found")?;
        let output = Command::new(bin)
            .arg("t")
            .arg("-y")
            .arg("-p-")
            .arg(archive_path.as_ref())
            .stdin(Stdio::null())
            .output()
            .map_err(|e| e.to_string())?;

        Ok(output.status.success())
    }

    pub fn delete<P: AsRef<Path>>(archive_path: P, file_names: &[String]) -> Result<(), String> {
        let bin = Self::get_rar_bin().ok_or("RAR binary not found")?;
        let mut cmd = Command::new(bin);
        cmd.arg("d").arg("-y").arg("-p-");
        cmd.arg(archive_path.as_ref());
        for f in file_names {
            cmd.arg(f);
        }
        cmd.stdin(Stdio::null());
        let output = cmd.output().map_err(|e| e.to_string())?;
        if output.status.success() {
            Ok(())
        } else {
            let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
            if err.is_empty() {
                Err(String::from_utf8_lossy(&output.stdout).trim().to_string())
            } else {
                Err(err)
            }
        }
    }

    pub fn rename<P: AsRef<Path>>(archive_path: P, old_name: &str, new_name: &str) -> Result<(), String> {
        let bin = Self::get_rar_bin().ok_or("RAR binary not found")?;
        let mut cmd = Command::new(bin);
        cmd.arg("rn")
            .arg("-y")
            .arg("-p-")
            .arg(archive_path.as_ref())
            .arg(old_name)
            .arg(new_name)
            .stdin(Stdio::null());

        let output = cmd.output().map_err(|e| e.to_string())?;
        if output.status.success() {
            Ok(())
        } else {
            let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
            if err.is_empty() {
                Err(String::from_utf8_lossy(&output.stdout).trim().to_string())
            } else {
                Err(err)
            }
        }
    }

    pub fn protect<P: AsRef<Path>>(archive_path: P, percent: &str) -> Result<(), String> {
        let bin = Self::get_rar_bin().ok_or("RAR binary not found")?;
        let clean_pct = percent.trim();
        let cmd_arg = if clean_pct.starts_with("rr") {
            clean_pct.to_string()
        } else {
            format!("rr{}", clean_pct)
        };

        let mut cmd = Command::new(bin);
        cmd.arg(cmd_arg)
            .arg("-y")
            .arg("-p-")
            .arg(archive_path.as_ref())
            .stdin(Stdio::null());

        let output = cmd.output().map_err(|e| e.to_string())?;
        if output.status.success() {
            Ok(())
        } else {
            let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
            if err.is_empty() {
                Err(String::from_utf8_lossy(&output.stdout).trim().to_string())
            } else {
                Err(err)
            }
        }
    }

    pub fn lock<P: AsRef<Path>>(archive_path: P) -> Result<(), String> {
        let bin = Self::get_rar_bin().ok_or("RAR binary not found")?;
        let mut cmd = Command::new(bin);
        cmd.arg("k")
            .arg("-y")
            .arg("-p-")
            .arg(archive_path.as_ref())
            .stdin(Stdio::null());

        let output = cmd.output().map_err(|e| e.to_string())?;
        if output.status.success() {
            Ok(())
        } else {
            let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
            if err.is_empty() {
                Err(String::from_utf8_lossy(&output.stdout).trim().to_string())
            } else {
                Err(err)
            }
        }
    }

    pub fn set_comment<P: AsRef<Path>>(archive_path: P, comment: &str) -> Result<(), String> {
        let bin = Self::get_rar_bin().ok_or("RAR binary not found")?;
        let tmp = std::env::temp_dir().join(format!("linrar_c_{}.txt", std::process::id()));
        fs::write(&tmp, comment).map_err(|e| e.to_string())?;

        let mut cmd = Command::new(bin);
        cmd.arg("c")
            .arg(format!("-z{}", tmp.to_string_lossy()))
            .arg("-y")
            .arg("-p-")
            .arg(archive_path.as_ref())
            .stdin(Stdio::null());

        let output = cmd.output().map_err(|e| e.to_string());
        let _ = fs::remove_file(tmp);

        let output = output?;
        if output.status.success() {
            Ok(())
        } else {
            let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
            if err.is_empty() {
                Err(String::from_utf8_lossy(&output.stdout).trim().to_string())
            } else {
                Err(err)
            }
        }
    }
}

