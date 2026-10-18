use std::fs::{self, File};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::types::{AddOptions, ArchiveEntry, ArchiveInfo, ArchiveType, CompressionLevel};

pub struct ZipEngine;

impl ZipEngine {
    pub fn read_entries<P: AsRef<Path>>(
        path: P,
    ) -> Result<(ArchiveInfo, Vec<ArchiveEntry>), String> {
        let path = path.as_ref();
        let file = File::open(path).map_err(|e| format!("Failed to open zip: {}", e))?;
        let mut archive =
            ZipArchive::new(file).map_err(|e| format!("Invalid zip format: {}", e))?;

        let mut entries = Vec::new();
        let mut total_unpacked = 0u64;
        let mut total_packed = 0u64;
        let mut total_files = 0;
        let mut total_folders = 0;

        for i in 0..archive.len() {
            if let Ok(file) = archive.by_index(i) {
                let is_dir = file.is_dir();
                let full_path = file.name().to_string();
                let name = full_path
                    .trim_end_matches('/')
                    .split('/')
                    .rfind(|s| !s.is_empty())
                    .unwrap_or(&full_path)
                    .to_string();

                let size = file.size();
                let packed_size = file.compressed_size();
                let crc = format!("{:08X}", file.crc32());
                let encrypted = file.encrypted();

                total_unpacked += size;
                total_packed += packed_size;
                if is_dir {
                    total_folders += 1;
                } else {
                    total_files += 1;
                }

                entries.push(ArchiveEntry {
                    name,
                    full_path,
                    size,
                    packed_size,
                    modified: "-".to_string(),
                    crc,
                    is_dir,
                    encrypted,
                    attributes: "-".to_string(),
                });
            }
        }

        let info = ArchiveInfo {
            path: path.to_path_buf(),
            archive_type: ArchiveType::Zip,
            total_files,
            total_folders,
            unpacked_size: total_unpacked,
            packed_size: total_packed,
            is_solid: false,
            is_locked: false,
            has_comment: false,
            comment: None,
        };

        Ok((info, entries))
    }

    pub fn extract<P: AsRef<Path>, D: AsRef<Path>>(
        archive_path: P,
        dest_dir: D,
        selected_files: Option<&[String]>,
    ) -> Result<(), String> {
        let file = File::open(archive_path.as_ref()).map_err(|e| e.to_string())?;
        let mut archive = ZipArchive::new(file).map_err(|e| e.to_string())?;
        let dest = dest_dir.as_ref();
        fs::create_dir_all(dest).map_err(|e| e.to_string())?;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i).map_err(|e| e.to_string())?;
            let outpath = match file.enclosed_name() {
                Some(path) => dest.join(path),
                None => continue,
            };

            if let Some(selected) = selected_files {
                let name = file.name();
                if !selected
                    .iter()
                    .any(|s| name == s || name.starts_with(&format!("{}/", s)))
                {
                    continue;
                }
            }

            if file.is_dir() {
                fs::create_dir_all(&outpath).map_err(|e| e.to_string())?;
            } else {
                if let Some(p) = outpath.parent()
                    && !p.exists()
                {
                    fs::create_dir_all(p).map_err(|e| e.to_string())?;
                }
                let mut outfile = File::create(&outpath).map_err(|e| e.to_string())?;
                std::io::copy(&mut file, &mut outfile).map_err(|e| e.to_string())?;
            }
        }

        Ok(())
    }

    pub fn create<P: AsRef<Path>>(
        archive_path: P,
        source_paths: &[PathBuf],
        options: &AddOptions,
    ) -> Result<(), String> {
        let file = File::create(archive_path.as_ref()).map_err(|e| e.to_string())?;
        let mut zip = ZipWriter::new(file);

        let method = match options.compression_level {
            CompressionLevel::Store => CompressionMethod::Stored,
            _ => CompressionMethod::Deflated,
        };

        let file_options = SimpleFileOptions::default()
            .compression_method(method)
            .unix_permissions(0o755);

        for src in source_paths {
            if !src.exists() {
                continue;
            }

            if src.is_file() {
                let filename = src
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                zip.start_file(filename, file_options)
                    .map_err(|e| e.to_string())?;
                let mut f = File::open(src).map_err(|e| e.to_string())?;
                std::io::copy(&mut f, &mut zip).map_err(|e| e.to_string())?;
            } else if src.is_dir() {
                let base_parent = src.parent().unwrap_or(src);
                for entry in WalkDir::new(src).into_iter().filter_map(|e| e.ok()) {
                    let path = entry.path();
                    let relative = path.strip_prefix(base_parent).unwrap_or(path);
                    let relative_str = relative.to_string_lossy().replace('\\', "/");

                    if entry.file_type().is_dir() {
                        zip.add_directory(format!("{}/", relative_str), file_options)
                            .map_err(|e| e.to_string())?;
                    } else {
                        zip.start_file(&relative_str, file_options)
                            .map_err(|e| e.to_string())?;
                        let mut f = File::open(path).map_err(|e| e.to_string())?;
                        std::io::copy(&mut f, &mut zip).map_err(|e| e.to_string())?;
                    }
                }
            }
        }

        if let Some(comment) = &options.comment {
            zip.set_raw_comment(comment.as_bytes().to_vec().into_boxed_slice());
        }

        zip.finish().map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn test<P: AsRef<Path>>(archive_path: P) -> Result<bool, String> {
        let file = File::open(archive_path.as_ref()).map_err(|e| e.to_string())?;
        let mut archive = ZipArchive::new(file).map_err(|e| e.to_string())?;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i).map_err(|e| e.to_string())?;
            let mut sink = std::io::sink();
            std::io::copy(&mut file, &mut sink).map_err(|e| e.to_string())?;
        }

        Ok(true)
    }
}
