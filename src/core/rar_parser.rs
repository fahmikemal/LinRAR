use std::fs::File;
use std::io::{Cursor, Read, Seek, SeekFrom};
use std::path::Path;
use crate::types::{ArchiveEntry, ArchiveInfo, ArchiveType};

pub struct RarParser;

impl RarParser {
    pub fn parse<P: AsRef<Path>>(path: P) -> Result<(ArchiveInfo, Vec<ArchiveEntry>), String> {
        let path = path.as_ref();
        let mut file = File::open(path).map_err(|e| format!("Failed to open file: {}", e))?;
        
        let mut magic = [0u8; 8];
        let n = file.read(&mut magic).map_err(|e| e.to_string())?;
        
        if n >= 8 && &magic[..8] == &[0x52, 0x61, 0x72, 0x21, 0x1A, 0x07, 0x01, 0x00] {
            Self::parse_rar5(&mut file, path)
        } else if n >= 7 && &magic[..7] == &[0x52, 0x61, 0x72, 0x21, 0x1A, 0x07, 0x00] {
            Self::parse_rar4(&mut file, path)
        } else {
            Err("Not a valid RAR archive signature".to_string())
        }
    }

    fn read_vint<R: Read>(reader: &mut R) -> std::io::Result<u64> {
        let mut result = 0u64;
        let mut shift = 0;
        loop {
            let mut byte = [0u8; 1];
            reader.read_exact(&mut byte)?;
            let b = byte[0];
            result |= ((b & 0x7F) as u64) << shift;
            if (b & 0x80) == 0 {
                break;
            }
            shift += 7;
            if shift >= 64 {
                break;
            }
        }
        Ok(result)
    }

    fn parse_rar5<R: Read + Seek>(reader: &mut R, path: &Path) -> Result<(ArchiveInfo, Vec<ArchiveEntry>), String> {
        reader.seek(SeekFrom::Start(8)).map_err(|e| e.to_string())?;

        let mut entries = Vec::new();
        let mut total_unpacked = 0u64;
        let mut total_packed = 0u64;
        let mut is_solid = false;
        let mut is_locked = false;
        let mut comment = None;

        let file_len = reader.seek(SeekFrom::End(0)).unwrap_or(0);
        reader.seek(SeekFrom::Start(8)).map_err(|e| e.to_string())?;

        while let Ok(cur_pos) = reader.stream_position() {
            if cur_pos >= file_len {
                break;
            }

            // CRC32 (4 bytes)
            let mut crc_buf = [0u8; 4];
            if reader.read_exact(&mut crc_buf).is_err() {
                break;
            }

            let header_size = match Self::read_vint(reader) {
                Ok(sz) if sz > 0 => sz,
                _ => break,
            };

            let header_start_pos = match reader.stream_position() {
                Ok(p) => p,
                Err(_) => break,
            };

            let mut header_data = vec![0u8; header_size as usize];
            if reader.read_exact(&mut header_data).is_err() {
                break;
            }

            let mut cur = Cursor::new(&header_data);
            let header_type = Self::read_vint(&mut cur).unwrap_or(0);
            let header_flags = Self::read_vint(&mut cur).unwrap_or(0);

            let has_extra = (header_flags & 0x0001) != 0;
            let has_data = (header_flags & 0x0002) != 0;

            if has_extra {
                let _extra_size = Self::read_vint(&mut cur).unwrap_or(0);
            }

            let data_size = if has_data {
                Self::read_vint(&mut cur).unwrap_or(0)
            } else {
                0
            };

            match header_type {
                1 => {
                    // Main Archive Header
                    let main_flags = Self::read_vint(&mut cur).unwrap_or(0);
                    if (main_flags & 0x0004) != 0 {
                        is_solid = true;
                    }
                    if (main_flags & 0x0010) != 0 {
                        is_locked = true;
                    }
                }
                2 => {
                    // File Header
                    let file_flags = Self::read_vint(&mut cur).unwrap_or(0);
                    let unpacked_size = Self::read_vint(&mut cur).unwrap_or(0);
                    let attributes = Self::read_vint(&mut cur).unwrap_or(0);

                    let is_dir = (file_flags & 0x0001) != 0;

                    // mtime (optional)
                    if (file_flags & 0x0002) != 0 {
                        let mut mtime_buf = [0u8; 4];
                        let _ = cur.read_exact(&mut mtime_buf);
                    }

                    // Data CRC32
                    let mut data_crc = [0u8; 4];
                    let _ = cur.read_exact(&mut data_crc);
                    let crc_hex = format!("{:02X}{:02X}{:02X}{:02X}", data_crc[3], data_crc[2], data_crc[1], data_crc[0]);

                    // Compression info & Host OS
                    let _comp_info = Self::read_vint(&mut cur).unwrap_or(0);
                    let _host_os = Self::read_vint(&mut cur).unwrap_or(0);

                    // Name length & Name
                    let name_len = Self::read_vint(&mut cur).unwrap_or(0) as usize;
                    let mut name_bytes = vec![0u8; name_len];
                    let _ = cur.read_exact(&mut name_bytes);
                    let full_name = String::from_utf8_lossy(&name_bytes).replace('\\', "/");
                    let file_name = full_name.split('/').filter(|s| !s.is_empty()).last().unwrap_or(&full_name).to_string();

                    total_unpacked += unpacked_size;
                    total_packed += data_size;

                    entries.push(ArchiveEntry {
                        name: file_name,
                        full_path: full_name,
                        size: unpacked_size,
                        packed_size: data_size,
                        modified: "-".to_string(),
                        crc: crc_hex,
                        is_dir,
                        encrypted: false,
                        attributes: format!("{:o}", attributes),
                    });
                }
                3 => {
                    // Service Header (comments, ACL, recovery, etc.)
                    // Check if comment service header
                    if data_size > 0 && data_size < 65536 {
                        let mut cmt_bytes = vec![0u8; data_size as usize];
                        let current_reader_pos = header_start_pos + header_size;
                        if reader.seek(SeekFrom::Start(current_reader_pos)).is_ok() {
                            if reader.read_exact(&mut cmt_bytes).is_ok() {
                                if let Ok(s) = String::from_utf8(cmt_bytes) {
                                    comment = Some(s);
                                }
                            }
                        }
                    }
                }
                _ => {}
            }

            // Advance reader past header and data payload
            let next_pos = header_start_pos + header_size + data_size;
            if reader.seek(SeekFrom::Start(next_pos)).is_err() {
                break;
            }
        }

        let total_files = entries.iter().filter(|e| !e.is_dir).count();
        let total_folders = entries.iter().filter(|e| e.is_dir).count();

        let info = ArchiveInfo {
            path: path.to_path_buf(),
            archive_type: ArchiveType::Rar5,
            total_files,
            total_folders,
            unpacked_size: total_unpacked,
            packed_size: total_packed,
            is_solid,
            is_locked,
            has_comment: comment.is_some(),
            comment,
        };

        Ok((info, entries))
    }

    fn parse_rar4<R: Read + Seek>(reader: &mut R, path: &Path) -> Result<(ArchiveInfo, Vec<ArchiveEntry>), String> {
        reader.seek(SeekFrom::Start(7)).map_err(|e| e.to_string())?;

        let mut entries = Vec::new();
        let mut total_unpacked = 0u64;
        let mut total_packed = 0u64;
        let mut is_solid = false;
        let mut is_locked = false;

        let file_len = reader.seek(SeekFrom::End(0)).unwrap_or(0);
        reader.seek(SeekFrom::Start(7)).map_err(|e| e.to_string())?;

        while let Ok(cur_pos) = reader.stream_position() {
            if cur_pos >= file_len {
                break;
            }

            // RAR4 Block Header: CRC (2 bytes), Type (1 byte), Flags (2 bytes), Size (2 bytes)
            let mut block_hdr = [0u8; 7];
            if reader.read_exact(&mut block_hdr).is_err() {
                break;
            }

            let block_type = block_hdr[2];
            let block_flags = u16::from_le_bytes([block_hdr[3], block_hdr[4]]);
            let mut block_size = u16::from_le_bytes([block_hdr[5], block_hdr[6]]) as u64;

            let add_size = if (block_flags & 0x8000) != 0 {
                let mut add_buf = [0u8; 4];
                if reader.read_exact(&mut add_buf).is_ok() {
                    block_size += 4;
                    u32::from_le_bytes(add_buf) as u64
                } else {
                    0
                }
            } else {
                0
            };

            let block_start = cur_pos;

            match block_type {
                0x73 => {
                    // Main archive header
                    if (block_flags & 0x0008) != 0 {
                        is_solid = true;
                    }
                    if (block_flags & 0x0004) != 0 {
                        is_locked = true;
                    }
                }
                0x74 => {
                    // File header
                    let mut file_hdr = [0u8; 21];
                    if reader.read_exact(&mut file_hdr).is_ok() {
                        let packed_size = u32::from_le_bytes([file_hdr[0], file_hdr[1], file_hdr[2], file_hdr[3]]) as u64;
                        let unpacked_size = u32::from_le_bytes([file_hdr[4], file_hdr[5], file_hdr[6], file_hdr[7]]) as u64;
                        let file_crc = u32::from_le_bytes([file_hdr[12], file_hdr[13], file_hdr[14], file_hdr[15]]);
                        let name_size = u16::from_le_bytes([file_hdr[19], file_hdr[20]]) as usize;

                        let is_dir = (block_flags & 0x00E0) == 0x00E0;

                        let mut name_buf = vec![0u8; name_size];
                        let _ = reader.read_exact(&mut name_buf);
                        let full_name = String::from_utf8_lossy(&name_buf).replace('\\', "/");
                        let file_name = full_name.split('/').filter(|s| !s.is_empty()).last().unwrap_or(&full_name).to_string();

                        total_unpacked += unpacked_size;
                        total_packed += packed_size;

                        entries.push(ArchiveEntry {
                            name: file_name,
                            full_path: full_name,
                            size: unpacked_size,
                            packed_size,
                            modified: "-".to_string(),
                            crc: format!("{:08X}", file_crc),
                            is_dir,
                            encrypted: (block_flags & 0x0004) != 0,
                            attributes: format!("{:x}", block_flags),
                        });
                    }
                }
                _ => {}
            }

            let next_pos = block_start + block_size + add_size;
            if reader.seek(SeekFrom::Start(next_pos)).is_err() {
                break;
            }
        }

        let total_files = entries.iter().filter(|e| !e.is_dir).count();
        let total_folders = entries.iter().filter(|e| e.is_dir).count();

        let info = ArchiveInfo {
            path: path.to_path_buf(),
            archive_type: ArchiveType::Rar4,
            total_files,
            total_folders,
            unpacked_size: total_unpacked,
            packed_size: total_packed,
            is_solid,
            is_locked,
            has_comment: false,
            comment: None,
        };

        Ok((info, entries))
    }
}
