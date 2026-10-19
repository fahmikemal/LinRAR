use std::collections::{HashSet, VecDeque};
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

use crate::types::{ArchiveEntry, ArchiveInfo, ArchiveType};

const SECTOR_SIZE: usize = 2048;
const PVD_SECTOR: u64 = 16;
const MAX_DESCRIPTOR_SECTORS: u64 = 32;

/// Internal representation of Volume Descriptor headers and root directory
#[derive(Debug, Clone)]
struct VolumeDescriptor {
    is_joliet: bool,
    volume_label: String,
    system_id: String,
    #[allow(dead_code)]
    volume_space_size: u64,
    root_lba: u32,
    root_data_len: u32,
}

#[derive(Debug, Clone)]
struct DirRecord {
    extent_lba: u32,
    data_len: u32,
    is_dir: bool,
    name: String,
    modified: String,
}

pub struct IsoEngine;

impl IsoEngine {
    /// Parses volume descriptors and locates the primary or Joliet descriptor
    fn parse_volume_descriptor<R: Read + Seek>(reader: &mut R) -> Result<VolumeDescriptor, String> {
        let mut pvd_desc: Option<VolumeDescriptor> = None;
        let mut joliet_desc: Option<VolumeDescriptor> = None;

        for sector in PVD_SECTOR..MAX_DESCRIPTOR_SECTORS {
            let offset = sector * (SECTOR_SIZE as u64);
            if reader.seek(SeekFrom::Start(offset)).is_err() {
                break;
            }

            let mut sector_buf = [0u8; SECTOR_SIZE];
            if reader.read_exact(&mut sector_buf).is_err() {
                break;
            }

            let desc_type = sector_buf[0];
            let standard_id = &sector_buf[1..6];

            if standard_id != b"CD001" {
                break;
            }

            if desc_type == 255 {
                // Volume Descriptor Set Terminator
                break;
            }

            if desc_type == 1 && pvd_desc.is_none() {
                // Primary Volume Descriptor
                let sys_id = String::from_utf8_lossy(&sector_buf[8..40])
                    .trim()
                    .trim_matches('\0')
                    .to_string();
                let vol_id = String::from_utf8_lossy(&sector_buf[40..72])
                    .trim()
                    .trim_matches('\0')
                    .to_string();
                let vol_space_size = u32::from_le_bytes([
                    sector_buf[80],
                    sector_buf[81],
                    sector_buf[82],
                    sector_buf[83],
                ]) as u64;

                let root_dr_len = sector_buf[156] as usize;
                if root_dr_len >= 34 {
                    let root_lba = u32::from_le_bytes([
                        sector_buf[158],
                        sector_buf[159],
                        sector_buf[160],
                        sector_buf[161],
                    ]);
                    let root_len = u32::from_le_bytes([
                        sector_buf[166],
                        sector_buf[167],
                        sector_buf[168],
                        sector_buf[169],
                    ]);

                    pvd_desc = Some(VolumeDescriptor {
                        is_joliet: false,
                        volume_label: vol_id,
                        system_id: sys_id,
                        volume_space_size: vol_space_size * (SECTOR_SIZE as u64),
                        root_lba,
                        root_data_len: root_len,
                    });
                }
            } else if desc_type == 2 && joliet_desc.is_none() {
                // Supplementary Volume Descriptor (Joliet Unicode UCS-2)
                let sys_chars: Vec<u16> = sector_buf[8..40]
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|&[b0, b1]| u16::from_be_bytes([b0, b1]))
                    .collect();
                let sys_id = String::from_utf16_lossy(&sys_chars)
                    .trim()
                    .trim_matches('\0')
                    .to_string();

                let vol_chars: Vec<u16> = sector_buf[40..72]
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|&[b0, b1]| u16::from_be_bytes([b0, b1]))
                    .collect();
                let vol_id = String::from_utf16_lossy(&vol_chars)
                    .trim()
                    .trim_matches('\0')
                    .to_string();

                let vol_space_size = u32::from_le_bytes([
                    sector_buf[80],
                    sector_buf[81],
                    sector_buf[82],
                    sector_buf[83],
                ]) as u64;

                let root_dr_len = sector_buf[156] as usize;
                if root_dr_len >= 34 {
                    let root_lba = u32::from_le_bytes([
                        sector_buf[158],
                        sector_buf[159],
                        sector_buf[160],
                        sector_buf[161],
                    ]);
                    let root_len = u32::from_le_bytes([
                        sector_buf[166],
                        sector_buf[167],
                        sector_buf[168],
                        sector_buf[169],
                    ]);

                    joliet_desc = Some(VolumeDescriptor {
                        is_joliet: true,
                        volume_label: vol_id,
                        system_id: sys_id,
                        volume_space_size: vol_space_size * (SECTOR_SIZE as u64),
                        root_lba,
                        root_data_len: root_len,
                    });
                }
            }
        }

        // Prefer Joliet for high-fidelity Unicode names, fall back to PVD
        if let Some(desc) = joliet_desc {
            Ok(desc)
        } else if let Some(desc) = pvd_desc {
            Ok(desc)
        } else {
            Err("Valid ISO 9660 volume descriptor not found".to_string())
        }
    }

    /// Decodes a 7-byte ISO 9660 date-time stamp into a human readable string
    fn parse_date_time(bytes: &[u8]) -> String {
        if bytes.len() < 6 {
            return "-".to_string();
        }
        let year = 1900 + bytes[0] as i32;
        let month = bytes[1];
        let day = bytes[2];
        let hour = bytes[3];
        let min = bytes[4];
        let sec = bytes[5];

        if (1..=12).contains(&month) && (1..=31).contains(&day) {
            format!(
                "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
                year, month, day, hour, min, sec
            )
        } else {
            "-".to_string()
        }
    }

    /// Parses directory records out of a directory byte buffer
    fn parse_directory_records(dir_bytes: &[u8], is_joliet: bool) -> Vec<DirRecord> {
        let mut records = Vec::new();
        let mut pos = 0;

        while pos < dir_bytes.len() {
            let record_len = dir_bytes[pos] as usize;
            if record_len == 0 {
                // Pad to next 2048 sector boundary
                let sector_off = pos % SECTOR_SIZE;
                if sector_off == 0 {
                    pos += SECTOR_SIZE;
                } else {
                    pos += SECTOR_SIZE - sector_off;
                }
                continue;
            }

            if record_len < 33 || pos + record_len > dir_bytes.len() {
                break;
            }

            let extent_lba = u32::from_le_bytes([
                dir_bytes[pos + 2],
                dir_bytes[pos + 3],
                dir_bytes[pos + 4],
                dir_bytes[pos + 5],
            ]);
            let data_len = u32::from_le_bytes([
                dir_bytes[pos + 10],
                dir_bytes[pos + 11],
                dir_bytes[pos + 12],
                dir_bytes[pos + 13],
            ]);

            let modified = Self::parse_date_time(&dir_bytes[pos + 18..pos + 25]);
            let flags = dir_bytes[pos + 25];
            let is_dir = (flags & 0x02) != 0;

            let len_fi = dir_bytes[pos + 32] as usize;
            if pos + 33 + len_fi > pos + record_len {
                pos += record_len;
                continue;
            }

            let fi_bytes = &dir_bytes[pos + 33..pos + 33 + len_fi];

            // Filter out current directory (.) and parent directory (..)
            let is_special = if is_joliet {
                (len_fi == 1 && (fi_bytes[0] == 0 || fi_bytes[0] == 1))
                    || (len_fi == 2 && fi_bytes[0] == 0 && (fi_bytes[1] == 0 || fi_bytes[1] == 1))
            } else {
                len_fi == 1 && (fi_bytes[0] == 0 || fi_bytes[0] == 1)
            };

            if is_special {
                pos += record_len;
                continue;
            }

            let mut name = if is_joliet {
                let chars: Vec<u16> = fi_bytes
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|&[b0, b1]| u16::from_be_bytes([b0, b1]))
                    .collect();
                String::from_utf16_lossy(&chars)
            } else {
                String::from_utf8_lossy(fi_bytes).to_string()
            };

            // Remove ISO 9660 semicolon version suffix (;1)
            if let Some(idx) = name.rfind(';') {
                name.truncate(idx);
            }
            // Remove trailing dot (ISO 9660 dot separator for files without extensions)
            if name.ends_with('.') {
                name.pop();
            }

            let trimmed_name = name.trim().to_string();
            if !trimmed_name.is_empty() {
                records.push(DirRecord {
                    extent_lba,
                    data_len,
                    is_dir,
                    name: trimmed_name,
                    modified,
                });
            }

            pos += record_len;
        }

        records
    }

    /// Reads an ISO image and returns metadata and the full entry hierarchy
    pub fn read_entries<P: AsRef<Path>>(
        path: P,
    ) -> Result<(ArchiveInfo, Vec<ArchiveEntry>), String> {
        let path = path.as_ref();
        let mut file = File::open(path).map_err(|e| format!("Failed to open ISO image: {}", e))?;
        let file_len = file.metadata().map(|m| m.len()).unwrap_or(0);

        let desc = Self::parse_volume_descriptor(&mut file)?;

        let mut entries = Vec::new();
        let mut total_files = 0;
        let mut total_folders = 0;
        let mut unpacked_size = 0u64;

        let mut queue: VecDeque<(u32, u32, String)> = VecDeque::new();
        let mut visited_lbas = HashSet::new();

        queue.push_back((desc.root_lba, desc.root_data_len, String::new()));
        visited_lbas.insert(desc.root_lba);

        while let Some((dir_lba, dir_len, current_rel_dir)) = queue.pop_front() {
            let offset = dir_lba as u64 * (SECTOR_SIZE as u64);
            if offset >= file_len || file.seek(SeekFrom::Start(offset)).is_err() {
                continue;
            }

            let read_len = (dir_len as usize).min((file_len - offset) as usize);
            let mut dir_buf = vec![0u8; read_len];
            if file.read_exact(&mut dir_buf).is_err() {
                continue;
            }

            let records = Self::parse_directory_records(&dir_buf, desc.is_joliet);

            for rec in records {
                let full_path = if current_rel_dir.is_empty() {
                    rec.name.clone()
                } else {
                    format!("{}/{}", current_rel_dir, rec.name)
                };

                if rec.is_dir {
                    total_folders += 1;
                    entries.push(ArchiveEntry {
                        name: rec.name.clone(),
                        full_path: full_path.clone(),
                        size: 0,
                        packed_size: 0,
                        modified: rec.modified.clone(),
                        crc: "-".to_string(),
                        is_dir: true,
                        encrypted: false,
                        attributes: "drwxr-xr-x".to_string(),
                    });

                    if visited_lbas.insert(rec.extent_lba) {
                        queue.push_back((rec.extent_lba, rec.data_len, full_path));
                    }
                } else {
                    total_files += 1;
                    unpacked_size += rec.data_len as u64;
                    entries.push(ArchiveEntry {
                        name: rec.name,
                        full_path,
                        size: rec.data_len as u64,
                        packed_size: rec.data_len as u64,
                        modified: rec.modified,
                        crc: "-".to_string(),
                        is_dir: false,
                        encrypted: false,
                        attributes: "-rw-r--r--".to_string(),
                    });
                }
            }
        }

        let format_desc = if desc.is_joliet {
            "ISO 9660 with Joliet Unicode Extensions"
        } else {
            "Standard ISO 9660"
        };

        let comment = if !desc.volume_label.is_empty() || !desc.system_id.is_empty() {
            Some(format!(
                "Volume ID: {}\nSystem: {}\nFormat: {}",
                if desc.volume_label.is_empty() {
                    "<None>"
                } else {
                    &desc.volume_label
                },
                if desc.system_id.is_empty() {
                    "<None>"
                } else {
                    &desc.system_id
                },
                format_desc
            ))
        } else {
            Some(format!("Format: {}", format_desc))
        };

        let info = ArchiveInfo {
            path: path.to_path_buf(),
            archive_type: ArchiveType::Iso,
            total_files,
            total_folders,
            unpacked_size,
            packed_size: file_len,
            is_solid: false,
            is_locked: false,
            has_comment: true,
            comment,
        };

        Ok((info, entries))
    }

    /// Extracts files from an ISO container image into `dest_dir`
    pub fn extract<P: AsRef<Path>, D: AsRef<Path>>(
        archive_path: P,
        dest_dir: D,
        selected_files: Option<&[String]>,
    ) -> Result<(), String> {
        let arch = archive_path.as_ref();
        let dest = dest_dir.as_ref();
        fs::create_dir_all(dest)
            .map_err(|e| format!("Failed to create destination directory: {}", e))?;

        let mut file = File::open(arch).map_err(|e| format!("Failed to open ISO image: {}", e))?;
        let file_len = file.metadata().map(|m| m.len()).unwrap_or(0);

        let desc = Self::parse_volume_descriptor(&mut file)?;

        let mut queue: VecDeque<(u32, u32, String)> = VecDeque::new();
        let mut visited_lbas = HashSet::new();

        queue.push_back((desc.root_lba, desc.root_data_len, String::new()));
        visited_lbas.insert(desc.root_lba);

        while let Some((dir_lba, dir_len, current_rel_dir)) = queue.pop_front() {
            let offset = dir_lba as u64 * (SECTOR_SIZE as u64);
            if offset >= file_len || file.seek(SeekFrom::Start(offset)).is_err() {
                continue;
            }

            let read_len = (dir_len as usize).min((file_len - offset) as usize);
            let mut dir_buf = vec![0u8; read_len];
            if file.read_exact(&mut dir_buf).is_err() {
                continue;
            }

            let records = Self::parse_directory_records(&dir_buf, desc.is_joliet);

            for rec in records {
                let full_path = if current_rel_dir.is_empty() {
                    rec.name.clone()
                } else {
                    format!("{}/{}", current_rel_dir, rec.name)
                };

                // Security check against directory traversal attacks
                let normalized = Path::new(&full_path);
                if normalized
                    .components()
                    .any(|c| matches!(c, std::path::Component::ParentDir))
                {
                    continue;
                }

                if rec.is_dir {
                    if visited_lbas.insert(rec.extent_lba) {
                        queue.push_back((rec.extent_lba, rec.data_len, full_path));
                    }
                } else {
                    // Filter by selected files if requested
                    if let Some(selected) = selected_files {
                        let is_match = selected
                            .iter()
                            .any(|s| full_path == *s || full_path.starts_with(&format!("{}/", s)));
                        if !is_match {
                            continue;
                        }
                    }

                    let target_path = dest.join(&full_path);
                    if let Some(parent) = target_path.parent() {
                        let _ = fs::create_dir_all(parent);
                    }

                    let extent_offset = rec.extent_lba as u64 * (SECTOR_SIZE as u64);
                    if extent_offset + rec.data_len as u64 > file_len {
                        return Err(format!(
                            "File extent for '{}' exceeds container bounds",
                            full_path
                        ));
                    }

                    file.seek(SeekFrom::Start(extent_offset))
                        .map_err(|e| format!("Failed to seek to extent: {}", e))?;

                    let mut out_file = File::create(&target_path).map_err(|e| {
                        format!("Failed to create extracted file '{}': {}", full_path, e)
                    })?;

                    let mut chunk_reader = (&mut file).take(rec.data_len as u64);
                    io::copy(&mut chunk_reader, &mut out_file).map_err(|e| {
                        format!("Failed to write extracted file '{}': {}", full_path, e)
                    })?;
                }
            }
        }

        Ok(())
    }

    /// Verifies data integrity and structural extents of the ISO image
    pub fn test<P: AsRef<Path>>(archive_path: P) -> Result<bool, String> {
        let arch = archive_path.as_ref();
        let mut file = File::open(arch).map_err(|e| format!("Failed to open ISO image: {}", e))?;
        let file_len = file.metadata().map(|m| m.len()).unwrap_or(0);

        let desc = Self::parse_volume_descriptor(&mut file)?;

        let mut queue: VecDeque<(u32, u32)> = VecDeque::new();
        let mut visited_lbas = HashSet::new();

        queue.push_back((desc.root_lba, desc.root_data_len));
        visited_lbas.insert(desc.root_lba);

        while let Some((dir_lba, dir_len)) = queue.pop_front() {
            let offset = dir_lba as u64 * (SECTOR_SIZE as u64);
            if offset + dir_len as u64 > file_len {
                return Err("Directory extent exceeds ISO file boundary".to_string());
            }

            file.seek(SeekFrom::Start(offset))
                .map_err(|e| format!("Seek failed: {}", e))?;

            let mut dir_buf = vec![0u8; dir_len as usize];
            file.read_exact(&mut dir_buf)
                .map_err(|e| format!("Read failed: {}", e))?;

            let records = Self::parse_directory_records(&dir_buf, desc.is_joliet);

            for rec in records {
                let extent_offset = rec.extent_lba as u64 * (SECTOR_SIZE as u64);
                if extent_offset + rec.data_len as u64 > file_len {
                    return Err(format!(
                        "File extent for '{}' exceeds container bounds",
                        rec.name
                    ));
                }

                if rec.is_dir {
                    if visited_lbas.insert(rec.extent_lba) {
                        queue.push_back((rec.extent_lba, rec.data_len));
                    }
                } else {
                    file.seek(SeekFrom::Start(extent_offset))
                        .map_err(|e| format!("Seek failed on '{}': {}", rec.name, e))?;

                    let mut sink = io::sink();
                    let mut reader = (&mut file).take(rec.data_len as u64);
                    io::copy(&mut reader, &mut sink)
                        .map_err(|e| format!("Failed to read extent for '{}': {}", rec.name, e))?;
                }
            }
        }

        Ok(true)
    }
}
