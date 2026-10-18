use crate::types::ArchiveType;
use std::fs::File;
use std::io::Read;
use std::path::Path;

const RAR5_MAGIC: &[u8] = &[0x52, 0x61, 0x72, 0x21, 0x1A, 0x07, 0x01, 0x00];
const RAR4_MAGIC: &[u8] = &[0x52, 0x61, 0x72, 0x21, 0x1A, 0x07, 0x00];
const ZIP_MAGIC: &[u8] = &[0x50, 0x4B, 0x03, 0x04];
const ZIP_EMPTY_MAGIC: &[u8] = &[0x50, 0x4B, 0x05, 0x06];
const SEVEN_ZIP_MAGIC: &[u8] = &[0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C];
const GZIP_MAGIC: &[u8] = &[0x1F, 0x8B];
const BZIP2_MAGIC: &[u8] = &[0x42, 0x5A, 0x68];

pub fn detect_archive_type<P: AsRef<Path>>(path: P) -> ArchiveType {
    let path = path.as_ref();
    let name_lower = path.to_string_lossy().to_lowercase();
    let from_extension = if name_lower.ends_with(".zip") {
        ArchiveType::Zip
    } else if name_lower.ends_with(".tar.gz") || name_lower.ends_with(".tgz") {
        ArchiveType::TarGz
    } else if name_lower.ends_with(".tar.bz2") || name_lower.ends_with(".tbz2") {
        ArchiveType::TarBz2
    } else if name_lower.ends_with(".tar") {
        ArchiveType::Tar
    } else if name_lower.ends_with(".rar") {
        ArchiveType::Rar5
    } else if name_lower.ends_with(".7z") {
        ArchiveType::SevenZip
    } else {
        ArchiveType::Unknown
    };

    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return from_extension,
    };

    let mut header = [0u8; 512];
    let bytes_read = match file.read(&mut header) {
        Ok(n) => n,
        Err(_) => return ArchiveType::Unknown,
    };

    if bytes_read >= 8 && header.starts_with(RAR5_MAGIC) {
        return ArchiveType::Rar5;
    }

    if bytes_read >= 7 && header.starts_with(RAR4_MAGIC) {
        return ArchiveType::Rar4;
    }

    if bytes_read >= 4 && (header.starts_with(ZIP_MAGIC) || header.starts_with(ZIP_EMPTY_MAGIC)) {
        return ArchiveType::Zip;
    }

    if bytes_read >= 6 && header.starts_with(SEVEN_ZIP_MAGIC) {
        return ArchiveType::SevenZip;
    }

    if bytes_read >= 2 && header.starts_with(GZIP_MAGIC) {
        let name_lower = path.to_string_lossy().to_lowercase();
        if name_lower.ends_with(".tar.gz") || name_lower.ends_with(".tgz") {
            return ArchiveType::TarGz;
        }
        return ArchiveType::TarGz;
    }

    if bytes_read >= 3 && header.starts_with(BZIP2_MAGIC) {
        let name_lower = path.to_string_lossy().to_lowercase();
        if name_lower.ends_with(".tar.bz2") || name_lower.ends_with(".tbz2") {
            return ArchiveType::TarBz2;
        }
        return ArchiveType::TarBz2;
    }

    // Check TAR ustar magic at offset 257
    if bytes_read >= 262 && &header[257..262] == b"ustar" {
        return ArchiveType::Tar;
    }

    let name_lower = path.to_string_lossy().to_lowercase();
    if name_lower.ends_with(".tar") {
        return ArchiveType::Tar;
    }

    // Scan for embedded RAR SFX in executable files up to 2MB
    if bytes_read >= 2
        && header.starts_with(b"MZ")
        && let Ok(mut sfx_scan) = File::open(path)
    {
        let mut buffer = vec![0u8; 1024 * 1024 * 2]; // 2MB
        if let Ok(n) = sfx_scan.read(&mut buffer) {
            if let Some(pos) = buffer[..n].windows(8).position(|w| w == RAR5_MAGIC) {
                let _ = pos;
                return ArchiveType::Rar5;
            }
            if let Some(pos) = buffer[..n].windows(7).position(|w| w == RAR4_MAGIC) {
                let _ = pos;
                return ArchiveType::Rar4;
            }
        }
    }

    ArchiveType::Unknown
}

pub fn is_supported_archive<P: AsRef<Path>>(path: P) -> bool {
    detect_archive_type(path) != ArchiveType::Unknown
}
