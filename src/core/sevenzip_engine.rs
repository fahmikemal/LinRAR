use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use sevenz_rust2::{
    ArchiveReader, ArchiveWriter, Password, decompress_with_extract_fn_and_password,
    default_entry_extract_fn, encoder_options,
};

use crate::core::bypass::SmartBypass;
use crate::core::volume::VolumeManager;
use crate::types::{AddOptions, ArchiveEntry, ArchiveInfo, ArchiveType, CompressionLevel};

pub struct SevenZipEngine;

impl SevenZipEngine {
    pub fn read_entries<P: AsRef<Path>>(
        path: P,
        password: Option<&str>,
    ) -> Result<(ArchiveInfo, Vec<ArchiveEntry>), String> {
        let path = path.as_ref();
        let file = File::open(path).map_err(|e| e.to_string())?;
        let packed_size = file.metadata().map(|m| m.len()).unwrap_or(0);

        let pwd = password.map(Password::from).unwrap_or_else(Password::empty);
        let reader = ArchiveReader::open(path, pwd).map_err(|e| e.to_string())?;
        let archive = reader.archive();

        let mut entries = Vec::new();
        let mut total_unpacked = 0u64;
        let mut total_files = 0;
        let mut total_folders = 0;

        for entry in &archive.files {
            let full_path = entry.name().to_string();
            let is_dir = entry.is_directory();
            let size = entry.size();
            let packed_entry_size = entry.compressed_size;

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

            let modified = if entry.has_last_modified_date {
                let st: SystemTime = entry.last_modified_date.into();
                let dt: chrono::DateTime<chrono::Utc> = st.into();
                dt.format("%Y-%m-%d %H:%M:%S").to_string()
            } else {
                "-".to_string()
            };

            let crc = if entry.has_crc {
                format!("{:08X}", entry.crc)
            } else {
                "-".to_string()
            };

            let attributes = if is_dir {
                "drwxr-xr-x".to_string()
            } else {
                "-rw-r--r--".to_string()
            };

            let is_encrypted = password.is_some();

            entries.push(ArchiveEntry {
                name,
                full_path,
                size,
                packed_size: packed_entry_size,
                modified,
                crc,
                is_dir,
                encrypted: is_encrypted && entry.has_stream,
                attributes,
            });
        }

        let info = ArchiveInfo {
            path: path.to_path_buf(),
            archive_type: ArchiveType::SevenZip,
            total_files,
            total_folders,
            unpacked_size: total_unpacked,
            packed_size,
            is_solid: true,
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
        let arch = archive_path.as_ref();
        let dest = dest_dir.as_ref();
        fs::create_dir_all(dest).map_err(|e| e.to_string())?;

        let file = File::open(arch).map_err(|e| e.to_string())?;
        let pwd = password.map(Password::from).unwrap_or_else(Password::empty);

        decompress_with_extract_fn_and_password(file, dest, pwd, |entry, reader, out_path| {
            let name = entry.name();
            if let Some(selected) = selected_files
                && !selected
                    .iter()
                    .any(|s| name == *s || name.starts_with(&format!("{}/", s)))
            {
                // Skip entry
                return Ok(true);
            }

            default_entry_extract_fn(entry, reader, out_path)
        })
        .map_err(|e| e.to_string())?;

        Ok(())
    }

    pub fn create<P: AsRef<Path>>(
        archive_path: P,
        source_paths: &[PathBuf],
        options: &AddOptions,
    ) -> Result<(), String> {
        let arch_ref = archive_path.as_ref();
        let mut writer = ArchiveWriter::create(arch_ref).map_err(|e| e.to_string())?;

        let lzma_level = match options.compression_level {
            CompressionLevel::Store => 0,
            CompressionLevel::Fastest => 1,
            CompressionLevel::Fast => 3,
            CompressionLevel::Normal => 5,
            CompressionLevel::Good => 7,
            CompressionLevel::Best => 9,
        };

        let mut methods = Vec::new();
        if let Some(pwd) = &options.password
            && !pwd.is_empty()
        {
            methods.push(encoder_options::AesEncoderOptions::new(pwd.as_str().into()).into());
            writer.set_encrypt_header(options.encrypt_headers);
        }

        methods.push(encoder_options::Lzma2Options::from_level(lzma_level).into());
        writer.set_content_methods(methods.clone());

        let mut collected_entries = Vec::new();

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
                let entry = sevenz_rust2::ArchiveEntry::from_path(src, filename);
                collected_entries.push((entry, Some(src.to_path_buf())));
            } else if src.is_dir() {
                let base_parent = src.parent().unwrap_or(src);
                for entry in walkdir::WalkDir::new(src)
                    .into_iter()
                    .filter_map(|e| e.ok())
                {
                    let path = entry.path();
                    let relative = path.strip_prefix(base_parent).unwrap_or(path);
                    let relative_str = relative.to_string_lossy().replace('\\', "/");
                    if relative_str.is_empty() {
                        continue;
                    }

                    let arch_entry = sevenz_rust2::ArchiveEntry::from_path(path, relative_str);
                    if path.is_dir() {
                        collected_entries.push((arch_entry, None));
                    } else {
                        collected_entries.push((arch_entry, Some(path.to_path_buf())));
                    }
                }
            }
        }

        if options.is_solid {
            let (dir_entries, file_entries): (Vec<_>, Vec<_>) = collected_entries
                .into_iter()
                .partition(|(entry, _)| entry.is_directory);

            for (dir_entry, _) in dir_entries {
                writer
                    .push_archive_entry(dir_entry, None::<File>)
                    .map_err(|e| e.to_string())?;
            }

            if !file_entries.is_empty() {
                let mut entries = Vec::with_capacity(file_entries.len());
                let mut readers = Vec::with_capacity(file_entries.len());

                for (entry, file_path) in file_entries {
                    if let Some(path) = file_path {
                        entries.push(entry);
                        readers.push(sevenz_rust2::SourceReader::new(LazyFile::new(path)));
                    }
                }

                writer
                    .push_archive_entries(entries, readers)
                    .map_err(|e| e.to_string())?;
            }
        } else {
            let lzma_methods = methods.clone();
            let mut copy_methods = Vec::new();
            if let Some(pwd) = &options.password
                && !pwd.is_empty()
            {
                copy_methods
                    .push(encoder_options::AesEncoderOptions::new(pwd.as_str().into()).into());
            }
            copy_methods.push(sevenz_rust2::EncoderConfiguration::new(
                sevenz_rust2::EncoderMethod::COPY,
            ));

            for (entry, file_path) in collected_entries {
                if entry.is_directory {
                    writer
                        .push_archive_entry(entry, None::<File>)
                        .map_err(|e| e.to_string())?;
                } else if let Some(path) = file_path {
                    if SmartBypass::should_bypass_with_options(&path, options) {
                        writer.set_content_methods(copy_methods.clone());
                    } else {
                        writer.set_content_methods(lzma_methods.clone());
                    }
                    let f = File::open(&path).map_err(|e| e.to_string())?;
                    writer
                        .push_archive_entry(entry, Some(f))
                        .map_err(|e| e.to_string())?;
                }
            }
        }

        writer.finish().map_err(|e| e.to_string())?;

        // If volume_size is specified, split into chunk volumes
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
        let pwd = password.map(Password::from).unwrap_or_else(Password::empty);

        let temp_dir = std::env::temp_dir();
        decompress_with_extract_fn_and_password(
            file,
            &temp_dir,
            pwd,
            |_entry, reader, _out_path| {
                let mut sink = std::io::sink();
                std::io::copy(reader, &mut sink)
                    .map_err(|e| sevenz_rust2::Error::Other(e.to_string().into()))?;
                Ok(true)
            },
        )
        .map_err(|e| e.to_string())?;

        Ok(true)
    }
}

struct LazyFile {
    path: PathBuf,
    file: Option<File>,
}

impl LazyFile {
    fn new(path: PathBuf) -> Self {
        Self { path, file: None }
    }
}

impl std::io::Read for LazyFile {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.file.is_none() {
            self.file = Some(File::open(&self.path)?);
        }
        self.file.as_mut().unwrap().read(buf)
    }
}
