use std::path::PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArchiveType {
    Rar5,
    Rar4,
    Zip,
    Tar,
    TarGz,
    TarBz2,
    SevenZip,
    Unknown,
}

impl ArchiveType {
    pub fn name(&self) -> &'static str {
        match self {
            ArchiveType::Rar5 => "RAR 5.0",
            ArchiveType::Rar4 => "RAR 4.x",
            ArchiveType::Zip => "ZIP",
            ArchiveType::Tar => "TAR",
            ArchiveType::TarGz => "TAR.GZ",
            ArchiveType::TarBz2 => "TAR.BZ2",
            ArchiveType::SevenZip => "7-Zip",
            ArchiveType::Unknown => "Unknown",
        }
    }

    #[allow(dead_code)]
    pub fn default_extension(&self) -> &'static str {
        match self {
            ArchiveType::Rar5 | ArchiveType::Rar4 => "rar",
            ArchiveType::Zip => "zip",
            ArchiveType::Tar => "tar",
            ArchiveType::TarGz => "tar.gz",
            ArchiveType::TarBz2 => "tar.bz2",
            ArchiveType::SevenZip => "7z",
            ArchiveType::Unknown => "bin",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveEntry {
    pub name: String,
    pub full_path: String,
    pub size: u64,
    pub packed_size: u64,
    pub modified: String,
    pub crc: String,
    pub is_dir: bool,
    pub encrypted: bool,
    pub attributes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveInfo {
    pub path: PathBuf,
    pub archive_type: ArchiveType,
    pub total_files: usize,
    pub total_folders: usize,
    pub unpacked_size: u64,
    pub packed_size: u64,
    pub is_solid: bool,
    pub is_locked: bool,
    pub has_comment: bool,
    pub comment: Option<String>,
}

impl ArchiveInfo {
    pub fn compression_ratio(&self) -> f32 {
        if self.unpacked_size == 0 {
            0.0
        } else {
            (self.packed_size as f32 / self.unpacked_size as f32) * 100.0
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompressionLevel {
    Store = 0,
    Fastest = 1,
    Fast = 2,
    Normal = 3,
    Good = 4,
    Best = 5,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddOptions {
    pub compression_level: CompressionLevel,
    pub password: Option<String>,
    pub encrypt_headers: bool,
    pub volume_size: Option<String>, // e.g., "10M", "100M"
    pub is_solid: bool,
    pub lock_archive: bool,
    pub comment: Option<String>,
}

impl Default for AddOptions {
    fn default() -> Self {
        Self {
            compression_level: CompressionLevel::Normal,
            password: None,
            encrypt_headers: false,
            volume_size: None,
            is_solid: false,
            lock_archive: false,
            comment: None,
        }
    }
}
