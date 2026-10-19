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
use xz2::read::XzDecoder;
use xz2::write::XzEncoder;

use crate::core::encoding::ArchiveEncoding;
use crate::core::volume::VolumeManager;
use crate::types::{
    AddOptions, ArchiveEntry, ArchiveInfo, ArchiveType, CompressionLevel, format_unix_mode,
};

pub enum TarWriter<W: Write> {
    Plain(W),
    Gz(GzEncoder<W>),
    Bz2(BzEncoder<W>),
    Xz(XzEncoder<W>),
    Zstd(zstd::stream::write::Encoder<'static, W>),
}

impl<W: Write> Write for TarWriter<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            TarWriter::Plain(w) => w.write(buf),
            TarWriter::Gz(w) => w.write(buf),
            TarWriter::Bz2(w) => w.write(buf),
            TarWriter::Xz(w) => w.write(buf),
            TarWriter::Zstd(w) => w.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            TarWriter::Plain(w) => w.flush(),
            TarWriter::Gz(w) => w.flush(),
            TarWriter::Bz2(w) => w.flush(),
            TarWriter::Xz(w) => w.flush(),
            TarWriter::Zstd(w) => w.flush(),
        }
    }
}

impl<W: Write> TarWriter<W> {
    pub fn finish(self) -> Result<(), String> {
        match self {
            TarWriter::Plain(mut w) => w.flush().map_err(|e| e.to_string()),
            TarWriter::Gz(w) => w.finish().map(|_| ()).map_err(|e| e.to_string()),
            TarWriter::Bz2(w) => w.finish().map(|_| ()).map_err(|e| e.to_string()),
            TarWriter::Xz(w) => w.finish().map(|_| ()).map_err(|e| e.to_string()),
            TarWriter::Zstd(w) => w.finish().map(|_| ()).map_err(|e| e.to_string()),
        }
    }
}

pub struct TarEngine;

impl TarEngine {
    fn get_reader(file: File, archive_type: ArchiveType) -> Result<Box<dyn Read>, String> {
        match archive_type {
            ArchiveType::TarGz => Ok(Box::new(GzDecoder::new(file))),
            ArchiveType::TarBz2 => Ok(Box::new(BzDecoder::new(file))),
            ArchiveType::TarXz => Ok(Box::new(XzDecoder::new(file))),
            ArchiveType::TarZst => match zstd::stream::read::Decoder::new(file) {
                Ok(mut d) => {
                    let _ = d.window_log_max(31);
                    Ok(Box::new(d))
                }
                Err(e) => Err(format!("Zstandard decoder error: {e}")),
            },
            _ => Ok(Box::new(file)),
        }
    }

    pub fn read_entries<P: AsRef<Path>>(
        path: P,
        archive_type: ArchiveType,
    ) -> Result<(ArchiveInfo, Vec<ArchiveEntry>), String> {
        Self::read_entries_with_encoding(path, archive_type, ArchiveEncoding::Auto)
    }

    pub fn read_entries_with_encoding<P: AsRef<Path>>(
        path: P,
        archive_type: ArchiveType,
        encoding: ArchiveEncoding,
    ) -> Result<(ArchiveInfo, Vec<ArchiveEntry>), String> {
        let path = path.as_ref();
        let file = File::open(path).map_err(|e| e.to_string())?;

        let mut entries = Vec::new();
        let mut total_unpacked = 0u64;
        let packed_size = file.metadata().map(|m| m.len()).unwrap_or(0);
        let mut total_files = 0;
        let mut total_folders = 0;

        let boxed_reader = Self::get_reader(file, archive_type)?;
        let mut archive = Archive::new(boxed_reader);
        let tar_entries = archive.entries().map_err(|e| e.to_string())?;

        for entry in tar_entries.flatten() {
            let raw_path = entry.path_bytes();
            let (name, full_path) = encoding.decode_path(&raw_path);
            let is_dir = entry.header().entry_type().is_dir()
                || raw_path.ends_with(b"/")
                || full_path.ends_with('/');
            let size = entry.header().size().unwrap_or(0);

            total_unpacked += size;
            if is_dir {
                total_folders += 1;
            } else {
                total_files += 1;
            }

            let modified = if let Ok(mtime) = entry.header().mtime() {
                if let Some(dt) = chrono::DateTime::from_timestamp(mtime as i64, 0) {
                    dt.format("%Y-%m-%d %H:%M:%S").to_string()
                } else {
                    "-".to_string()
                }
            } else {
                "-".to_string()
            };

            let attributes = if let Ok(mode) = entry.header().mode() {
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
                packed_size: size, // TAR doesn't track per-file compressed size
                modified,
                crc: "-".to_string(),
                is_dir,
                encrypted: false,
                attributes,
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
        Self::extract_with_encoding(
            archive_path,
            dest_dir,
            archive_type,
            selected_files,
            ArchiveEncoding::Auto,
        )
    }

    pub fn extract_with_encoding<P: AsRef<Path>, D: AsRef<Path>>(
        archive_path: P,
        dest_dir: D,
        archive_type: ArchiveType,
        selected_files: Option<&[String]>,
        encoding: ArchiveEncoding,
    ) -> Result<(), String> {
        let file = File::open(archive_path.as_ref()).map_err(|e| e.to_string())?;
        let dest = dest_dir.as_ref();
        fs::create_dir_all(dest).map_err(|e| e.to_string())?;

        let boxed_reader = Self::get_reader(file, archive_type)?;
        let mut archive = Archive::new(boxed_reader);
        let entries = archive.entries().map_err(|e| e.to_string())?;

        for entry in entries {
            let mut entry = entry.map_err(|e| e.to_string())?;
            let raw_path = entry.path_bytes();
            let (name, full_path) = encoding.decode_path(&raw_path);

            let clean_relative = Path::new(&full_path);
            if clean_relative.components().any(|c| {
                matches!(
                    c,
                    std::path::Component::ParentDir
                        | std::path::Component::RootDir
                        | std::path::Component::Prefix(_)
                )
            }) {
                continue;
            }

            if let Some(selected) = selected_files {
                let matches_sel = selected.iter().any(|s| {
                    full_path == *s || full_path.starts_with(&format!("{}/", s)) || name == *s
                });
                if !matches_sel {
                    continue;
                }
            }

            let outpath = dest.join(clean_relative);
            let is_dir = entry.header().entry_type().is_dir()
                || raw_path.ends_with(b"/")
                || full_path.ends_with('/');
            if is_dir {
                fs::create_dir_all(&outpath).map_err(|e| e.to_string())?;
            } else {
                if let Some(p) = outpath.parent()
                    && !p.exists()
                {
                    fs::create_dir_all(p).map_err(|e| e.to_string())?;
                }
                let mut outfile = File::create(&outpath).map_err(|e| e.to_string())?;
                std::io::copy(&mut entry, &mut outfile).map_err(|e| e.to_string())?;

                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Ok(mode) = entry.header().mode() {
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
        archive_type: ArchiveType,
        options: &AddOptions,
    ) -> Result<(), String> {
        let arch_ref = archive_path.as_ref();
        let file = File::create(arch_ref).map_err(|e| e.to_string())?;

        let gz_comp = match options.compression_level {
            CompressionLevel::Store | CompressionLevel::Fastest => GzCompression::fast(),
            CompressionLevel::Good | CompressionLevel::Best => GzCompression::best(),
            _ => GzCompression::default(),
        };

        let bz_comp = match options.compression_level {
            CompressionLevel::Store | CompressionLevel::Fastest => BzCompression::fast(),
            CompressionLevel::Good | CompressionLevel::Best => BzCompression::best(),
            _ => BzCompression::default(),
        };

        let xz_level = match options.compression_level {
            CompressionLevel::Store | CompressionLevel::Fastest => 1,
            CompressionLevel::Fast => 2,
            CompressionLevel::Normal => 6,
            CompressionLevel::Good => 7,
            CompressionLevel::Best => 9,
        };

        let zstd_level = match options.compression_level {
            CompressionLevel::Store | CompressionLevel::Fastest => 1,
            CompressionLevel::Fast => 2,
            CompressionLevel::Normal => 3,
            CompressionLevel::Good => 9,
            CompressionLevel::Best => 19,
        };

        let tar_writer = match archive_type {
            ArchiveType::TarGz => TarWriter::Gz(GzEncoder::new(file, gz_comp)),
            ArchiveType::TarBz2 => TarWriter::Bz2(BzEncoder::new(file, bz_comp)),
            ArchiveType::TarXz => TarWriter::Xz(XzEncoder::new(file, xz_level)),
            ArchiveType::TarZst => match zstd::stream::write::Encoder::new(file, zstd_level) {
                Ok(mut enc) => {
                    let _ = enc.set_parameter(zstd::zstd_safe::CParameter::NbWorkers(
                        rayon::current_num_threads() as u32,
                    ));
                    if let Ok(Some(ldm_cfg)) = crate::core::ldm::LdmConfig::from_dict_str(
                        options.dict_size.as_deref(),
                        options.ldm,
                    ) {
                        let _ = ldm_cfg.apply_to_zstd(&mut enc);
                    }
                    TarWriter::Zstd(enc)
                }
                Err(e) => return Err(e.to_string()),
            },
            _ => TarWriter::Plain(file),
        };

        let mut builder = Builder::new(tar_writer);

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
        let writer = builder.into_inner().map_err(|e| e.to_string())?;
        writer.finish()?;

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

    pub fn test<P: AsRef<Path>>(
        archive_path: P,
        archive_type: ArchiveType,
    ) -> Result<bool, String> {
        let file = File::open(archive_path.as_ref()).map_err(|e| e.to_string())?;
        let boxed_reader = Self::get_reader(file, archive_type)?;
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
