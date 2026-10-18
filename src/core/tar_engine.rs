use bzip2::Compression as BzCompression;
use bzip2::read::BzDecoder;
use bzip2::write::BzEncoder;
use flate2::Compression as GzCompression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use tar::{Archive, Builder};

use crate::types::{AddOptions, ArchiveEntry, ArchiveInfo, ArchiveType};

pub struct TarEngine;

impl TarEngine {
    pub fn read_entries<P: AsRef<Path>>(
        path: P,
        archive_type: ArchiveType,
    ) -> Result<(ArchiveInfo, Vec<ArchiveEntry>), String> {
        let path = path.as_ref();
        let file = File::open(path).map_err(|e| e.to_string())?;

        let mut entries = Vec::new();
        let mut total_unpacked = 0u64;
        let packed_size = file.metadata().map(|m| m.len()).unwrap_or(0);
        let mut total_files = 0;
        let mut total_folders = 0;

        let boxed_reader: Box<dyn Read> = match archive_type {
            ArchiveType::TarGz => Box::new(GzDecoder::new(file)),
            ArchiveType::TarBz2 => Box::new(BzDecoder::new(file)),
            _ => Box::new(file),
        };

        let mut archive = Archive::new(boxed_reader);
        let tar_entries = archive.entries().map_err(|e| e.to_string())?;

        for entry in tar_entries.flatten() {
            let path_buf = entry.path().map_err(|e| e.to_string())?.into_owned();
            let full_path = path_buf.to_string_lossy().to_string();
            let is_dir = entry.header().entry_type().is_dir();
            let size = entry.header().size().unwrap_or(0);

            let name = full_path
                .trim_end_matches('/')
                .split('/')
                .rfind(|s| !s.is_empty())
                .unwrap_or(&full_path)
                .to_string();

            total_unpacked += size;
            if is_dir {
                total_folders += 1;
            } else {
                total_files += 1;
            }

            entries.push(ArchiveEntry {
                name,
                full_path,
                size,
                packed_size: size, // TAR doesn't track per-file compressed size
                modified: "-".to_string(),
                crc: "-".to_string(),
                is_dir,
                encrypted: false,
                attributes: format!("{:o}", entry.header().mode().unwrap_or(0)),
            });
        }

        let info = ArchiveInfo {
            path: path.to_path_buf(),
            archive_type,
            total_files,
            total_folders,
            unpacked_size: total_unpacked,
            packed_size,
            is_solid: archive_type != ArchiveType::Tar,
            is_locked: false,
            has_comment: false,
            comment: None,
        };

        Ok((info, entries))
    }

    pub fn extract<P: AsRef<Path>, D: AsRef<Path>>(
        archive_path: P,
        dest_dir: D,
        archive_type: ArchiveType,
        selected_files: Option<&[String]>,
    ) -> Result<(), String> {
        let file = File::open(archive_path.as_ref()).map_err(|e| e.to_string())?;
        let dest = dest_dir.as_ref();
        fs::create_dir_all(dest).map_err(|e| e.to_string())?;

        let boxed_reader: Box<dyn Read> = match archive_type {
            ArchiveType::TarGz => Box::new(GzDecoder::new(file)),
            ArchiveType::TarBz2 => Box::new(BzDecoder::new(file)),
            _ => Box::new(file),
        };

        let mut archive = Archive::new(boxed_reader);
        let tar_entries = archive.entries().map_err(|e| e.to_string())?;

        for mut entry in tar_entries.filter_map(|e| e.ok()) {
            let path_buf = match entry.path() {
                Ok(p) => p.into_owned(),
                Err(_) => continue,
            };
            let path_str = path_buf.to_string_lossy();

            if let Some(selected) = selected_files
                && !selected
                    .iter()
                    .any(|s| path_str == *s || path_str.starts_with(&format!("{}/", s)))
            {
                continue;
            }

            let outpath = dest.join(&path_buf);
            if entry.header().entry_type().is_dir() {
                fs::create_dir_all(&outpath).map_err(|e| e.to_string())?;
            } else {
                if let Some(p) = outpath.parent() {
                    fs::create_dir_all(p).map_err(|e| e.to_string())?;
                }
                entry.unpack(&outpath).map_err(|e| e.to_string())?;
            }
        }

        Ok(())
    }

    pub fn create<P: AsRef<Path>>(
        archive_path: P,
        source_paths: &[PathBuf],
        archive_type: ArchiveType,
        _options: &AddOptions,
    ) -> Result<(), String> {
        let file = File::create(archive_path.as_ref()).map_err(|e| e.to_string())?;

        let boxed_writer: Box<dyn Write> = match archive_type {
            ArchiveType::TarGz => Box::new(GzEncoder::new(file, GzCompression::default())),
            ArchiveType::TarBz2 => Box::new(BzEncoder::new(file, BzCompression::default())),
            _ => Box::new(file),
        };

        let mut builder = Builder::new(boxed_writer);

        for src in source_paths {
            if !src.exists() {
                continue;
            }

            let name = src.file_name().unwrap_or_default();
            if src.is_file() {
                let mut f = File::open(src).map_err(|e| e.to_string())?;
                builder
                    .append_file(name, &mut f)
                    .map_err(|e| e.to_string())?;
            } else if src.is_dir() {
                builder
                    .append_dir_all(name, src)
                    .map_err(|e| e.to_string())?;
            }
        }

        builder.finish().map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn test<P: AsRef<Path>>(
        archive_path: P,
        archive_type: ArchiveType,
    ) -> Result<bool, String> {
        let file = File::open(archive_path.as_ref()).map_err(|e| e.to_string())?;

        let boxed_reader: Box<dyn Read> = match archive_type {
            ArchiveType::TarGz => Box::new(GzDecoder::new(file)),
            ArchiveType::TarBz2 => Box::new(BzDecoder::new(file)),
            _ => Box::new(file),
        };

        let mut archive = Archive::new(boxed_reader);
        let tar_entries = archive.entries().map_err(|e| e.to_string())?;

        for entry in tar_entries {
            let mut entry = entry.map_err(|e| e.to_string())?;
            let mut sink = std::io::sink();
            std::io::copy(&mut entry, &mut sink).map_err(|e| e.to_string())?;
        }

        Ok(true)
    }
}
