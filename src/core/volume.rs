use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

#[allow(dead_code)]
pub struct VolumeManager;

#[allow(dead_code)]
impl VolumeManager {
    /// Parse human volume sizes like "10M", "100M", "700M", "1G", "4480M", "500k", or raw bytes.
    pub fn parse_size(size_str: &str) -> Result<u64, String> {
        let s = size_str.trim();
        if s.is_empty() {
            return Err("Empty volume size".to_string());
        }
        let alpha_idx = s.find(|c: char| c.is_alphabetic()).unwrap_or(s.len());
        let (num_str, unit) = s.split_at(alpha_idx);
        let num: f64 = num_str
            .trim()
            .parse()
            .map_err(|e| format!("Invalid number in volume size: {}", e))?;
        let mult = match unit.trim().to_uppercase().as_str() {
            "" | "B" => 1.0,
            "K" | "KB" => 1024.0,
            "M" | "MB" => 1024.0 * 1024.0,
            "G" | "GB" => 1024.0 * 1024.0 * 1024.0,
            other => return Err(format!("Unknown unit '{}' in volume size", other)),
        };
        Ok((num * mult) as u64)
    }

    /// Split an existing file/archive into multiple chunk volumes:
    /// e.g. "archive.zip.001", "archive.zip.002", etc.
    pub fn split_file<P: AsRef<Path>>(
        source_path: P,
        chunk_size_bytes: u64,
        output_prefix: Option<&Path>,
    ) -> Result<Vec<PathBuf>, String> {
        let src = source_path.as_ref();
        if !src.exists() {
            return Err("Source file does not exist".to_string());
        }
        if chunk_size_bytes < 1024 {
            return Err("Volume size must be at least 1 KB".to_string());
        }

        let mut file = File::open(src).map_err(|e| e.to_string())?;
        let total_size = file.metadata().map_err(|e| e.to_string())?.len();
        if total_size <= chunk_size_bytes {
            return Ok(vec![src.to_path_buf()]);
        }

        let parent = output_prefix
            .and_then(|p| p.parent())
            .or_else(|| src.parent())
            .unwrap_or(Path::new("."));
        let base_name = src.file_name().unwrap_or_default().to_string_lossy();

        let mut chunk_index = 1;
        let mut buffer = vec![0u8; 64 * 1024]; // 64KB buffer
        let mut created_parts = Vec::new();

        loop {
            let part_name = format!("{}.{:03}", base_name, chunk_index);
            let part_path = parent.join(&part_name);
            let mut part_file = File::create(&part_path).map_err(|e| e.to_string())?;

            let mut written_for_chunk = 0u64;
            let mut reached_eof = false;

            while written_for_chunk < chunk_size_bytes {
                let bytes_to_read =
                    ((chunk_size_bytes - written_for_chunk) as usize).min(buffer.len());
                let n = file
                    .read(&mut buffer[..bytes_to_read])
                    .map_err(|e| e.to_string())?;
                if n == 0 {
                    reached_eof = true;
                    break;
                }
                part_file
                    .write_all(&buffer[..n])
                    .map_err(|e| e.to_string())?;
                written_for_chunk += n as u64;
            }

            created_parts.push(part_path);
            if reached_eof {
                break;
            }
            chunk_index += 1;
        }

        Ok(created_parts)
    }

    /// Detect if a given file is part of a multi-volume set (e.g. .001, .part01.rar, .z01)
    /// and find all contiguous parts in the same directory.
    pub fn find_volume_parts<P: AsRef<Path>>(first_part: P) -> Result<Vec<PathBuf>, String> {
        let p = first_part.as_ref();
        if !p.exists() {
            return Err("File does not exist".to_string());
        }
        let parent = p.parent().unwrap_or(Path::new("."));
        let file_name = p.file_name().unwrap_or_default().to_string_lossy();

        let mut parts = Vec::new();

        // Pattern 1: name.001, name.002, ...
        if let Some(prefix) = file_name.strip_suffix(".001") {
            let mut idx = 1;
            loop {
                let candidate = parent.join(format!("{}.{:03}", prefix, idx));
                if candidate.exists() {
                    parts.push(candidate);
                    idx += 1;
                } else {
                    break;
                }
            }
        }
        // Pattern 2: name.part01.rar, name.part02.rar ... or name.part1.rar
        else if file_name.contains(".part") && file_name.ends_with(".rar") {
            if let Ok(entries) = std::fs::read_dir(parent) {
                let mut matches: Vec<PathBuf> = entries
                    .filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|path| {
                        let name = path.file_name().unwrap_or_default().to_string_lossy();
                        name.contains(".part") && name.ends_with(".rar")
                    })
                    .collect();
                matches.sort();
                parts = matches;
            }
        } else {
            parts.push(p.to_path_buf());
        }

        Ok(parts)
    }

    /// Join multiple chunk volumes into a single reconstructed file.
    pub fn join_files<P: AsRef<Path>, O: AsRef<Path>>(
        parts: &[P],
        output_path: O,
    ) -> Result<PathBuf, String> {
        if parts.is_empty() {
            return Err("No volume parts provided".to_string());
        }
        let out_path = output_path.as_ref();
        let mut outfile = File::create(out_path).map_err(|e| e.to_string())?;

        let mut buffer = vec![0u8; 64 * 1024];
        for part in parts {
            let mut part_file = File::open(part.as_ref())
                .map_err(|e| format!("Failed to open part {}: {}", part.as_ref().display(), e))?;
            loop {
                let n = part_file.read(&mut buffer).map_err(|e| e.to_string())?;
                if n == 0 {
                    break;
                }
                outfile.write_all(&buffer[..n]).map_err(|e| e.to_string())?;
            }
        }
        outfile.flush().map_err(|e| e.to_string())?;
        Ok(out_path.to_path_buf())
    }
}
