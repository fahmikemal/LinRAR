use std::fs::{self, File};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::core::bypass::SmartBypass;
use crate::core::volume::VolumeManager;
use crate::types::{
    AddOptions, ArchiveEntry, ArchiveInfo, ArchiveType, CompressionLevel, format_unix_mode,
};

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
            if let Ok(file) = archive.by_index_raw(i) {
                let is_dir = file.is_dir();
                let full_path = file.name().map(|n| n.to_string()).unwrap_or_default();
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

                let modified = if let Some(dt) = file.last_modified() {
                    format!(
                        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
                        dt.year(),
                        dt.month(),
                        dt.day(),
                        dt.hour(),
                        dt.minute(),
                        dt.second()
                    )
                } else {
                    "-".to_string()
                };

                let attributes = if let Some(mode) = file.unix_mode() {
                    format_unix_mode(mode, is_dir)
                } else if is_dir {
                    "drwxr-xr-x".to_string()
                } else {
                    "-rw-r--r--".to_string()
                };

                entries.push(ArchiveEntry {
                    name,
                    full_path,
                    size,
                    packed_size,
                    modified,
                    crc,
                    is_dir,
                    encrypted,
                    attributes,
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
        password: Option<&str>,
    ) -> Result<(), String> {
        let file = File::open(archive_path.as_ref()).map_err(|e| e.to_string())?;
        let mut archive = ZipArchive::new(file).map_err(|e| e.to_string())?;
        let dest = dest_dir.as_ref();
        fs::create_dir_all(dest).map_err(|e| e.to_string())?;

        for i in 0..archive.len() {
            let mut file = if let Some(pwd) = password.filter(|p| !p.is_empty()) {
                archive.by_index_decrypt(i, pwd.as_bytes())
            } else {
                archive.by_index(i)
            }
            .map_err(|e| e.to_string())?;

            let outpath = match file.enclosed_name() {
                Some(path) => dest.join(path),
                None => continue,
            };

            if let Some(selected) = selected_files {
                let name = file.name().map(|n| n.to_string()).unwrap_or_default();
                if !selected
                    .iter()
                    .any(|s| name == *s || name.starts_with(&format!("{}/", s)))
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

                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Some(mode) = file.unix_mode() {
                        let _ = fs::set_permissions(&outpath, fs::Permissions::from_mode(mode));
                    }
                }
            }
        }

        Ok(())
    }

    pub fn create<P: AsRef<Path>>(
        archive_path: P,
        source_paths: &[PathBuf],
        options: &AddOptions,
    ) -> Result<(), String> {
        let arch_ref = archive_path.as_ref();
        let file = File::create(arch_ref).map_err(|e| e.to_string())?;
        let mut zip = ZipWriter::new(file);

        let method = match options.compression_level {
            CompressionLevel::Store => CompressionMethod::Stored,
            _ => CompressionMethod::Deflated,
        };

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

                #[cfg(unix)]
                let permissions = {
                    use std::os::unix::fs::MetadataExt;
                    src.metadata().map(|m| m.mode()).unwrap_or(0o644)
                };
                #[cfg(not(unix))]
                let permissions = 0o644;

                let file_method = if SmartBypass::should_bypass_with_options(src, options) {
                    CompressionMethod::Stored
                } else {
                    method
                };

                let mut file_options = SimpleFileOptions::default()
                    .compression_method(file_method)
                    .unix_permissions(permissions);

                if let Some(pwd) = &options.password
                    && !pwd.is_empty()
                {
                    file_options = file_options.with_aes_encryption(zip::AesMode::Aes256, pwd);
                }

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

                    #[cfg(unix)]
                    let permissions = {
                        use std::os::unix::fs::MetadataExt;
                        entry.metadata().map(|m| m.mode()).unwrap_or(
                            if entry.file_type().is_dir() {
                                0o755
                            } else {
                                0o644
                            },
                        )
                    };
                    #[cfg(not(unix))]
                    let permissions = if entry.file_type().is_dir() {
                        0o755
                    } else {
                        0o644
                    };

                    let file_method = if entry.file_type().is_file()
                        && SmartBypass::should_bypass_with_options(path, options)
                    {
                        CompressionMethod::Stored
                    } else {
                        method
                    };

                    let mut file_options = SimpleFileOptions::default()
                        .compression_method(file_method)
                        .unix_permissions(permissions);

                    if let Some(pwd) = &options.password
                        && !pwd.is_empty()
                    {
                        file_options = file_options.with_aes_encryption(zip::AesMode::Aes256, pwd);
                    }

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
            let _ = zip.set_raw_comment(comment.as_bytes().to_vec().into_boxed_slice());
        }

        zip.finish().map_err(|e| e.to_string())?;

        // If volume_size is specified, split the created archive into chunks
        if let Some(vol_str) = &options.volume_size
            && !vol_str.trim().is_empty()
        {
            let chunk_size = VolumeManager::parse_size(vol_str)?;
            let parts = VolumeManager::split_file(arch_ref, chunk_size, None)?;
            if parts.len() > 1 {
                let _ = fs::remove_file(arch_ref);
            }
        }

        Ok(())
    }

    pub fn test<P: AsRef<Path>>(archive_path: P, password: Option<&str>) -> Result<bool, String> {
        let file = File::open(archive_path.as_ref()).map_err(|e| e.to_string())?;
        let mut archive = ZipArchive::new(file).map_err(|e| e.to_string())?;

        for i in 0..archive.len() {
            let mut file = if let Some(pwd) = password.filter(|p| !p.is_empty()) {
                archive.by_index_decrypt(i, pwd.as_bytes())
            } else {
                archive.by_index(i)
            }
            .map_err(|e| e.to_string())?;
            let mut sink = std::io::sink();
            std::io::copy(&mut file, &mut sink).map_err(|e| e.to_string())?;
        }

        Ok(true)
    }
}
