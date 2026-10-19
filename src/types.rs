use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArchiveType {
    Rar5,
    Rar4,
    Zip,
    Tar,
    TarGz,
    TarBz2,
    TarXz,
    TarZst,
    SevenZip,
    Iso,
    Sfx,
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
            ArchiveType::TarXz => "TAR.XZ",
            ArchiveType::TarZst => "TAR.ZST",
            ArchiveType::SevenZip => "7-Zip",
            ArchiveType::Iso => "ISO Image",
            ArchiveType::Sfx => "SFX Executable",
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
            ArchiveType::TarXz => "tar.xz",
            ArchiveType::TarZst => "tar.zst",
            ArchiveType::SevenZip => "7z",
            ArchiveType::Iso => "iso",
            ArchiveType::Sfx => "sfx",
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
    pub smart_bypass: bool,
    pub bypass_extensions: Option<Vec<String>>,
    pub delete_after_archiving: bool,
    pub shred_after_archiving: bool,
    pub shred_method: crate::core::shredder::ShredMethod,
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
            smart_bypass: true,
            bypass_extensions: None,
            delete_after_archiving: false,
            shred_after_archiving: false,
            shred_method: crate::core::shredder::ShredMethod::QuickZero,
        }
    }
}

pub fn format_unix_mode(mode: u32, is_dir: bool) -> String {
    let type_char = if is_dir { 'd' } else { '-' };
    let r1 = if mode & 0o400 != 0 { 'r' } else { '-' };
    let w1 = if mode & 0o200 != 0 { 'w' } else { '-' };
    let x1 = if mode & 0o100 != 0 { 'x' } else { '-' };
    let r2 = if mode & 0o040 != 0 { 'r' } else { '-' };
    let w2 = if mode & 0o020 != 0 { 'w' } else { '-' };
    let x2 = if mode & 0o010 != 0 { 'x' } else { '-' };
    let r3 = if mode & 0o004 != 0 { 'r' } else { '-' };
    let w3 = if mode & 0o002 != 0 { 'w' } else { '-' };
    let x3 = if mode & 0o001 != 0 { 'x' } else { '-' };
    format!(
        "{}{}{}{}{}{}{}{}{}{}",
        type_char, r1, w1, x1, r2, w2, x2, r3, w3, x3
    )
}
