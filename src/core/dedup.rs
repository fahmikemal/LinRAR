use fastcdc::v2020::{ChunkData, StreamCDC};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use walkdir::WalkDir;

use crate::core::blake3_integrity::{compute_merkle_root, hash_bytes, hash_to_hex};
use crate::types::{AddOptions, ArchiveEntry, ArchiveInfo, ArchiveType, CompressionLevel};

pub const DEDUP_MAGIC_HEADER: &[u8; 8] = b"LINRDED1";
pub const DEDUP_MAGIC_FOOTER: &[u8; 8] = b"LRDEDEND";

/// Content-Defined Chunking default settings
pub const DEFAULT_MIN_CHUNK: usize = 8192; // 8 KB
pub const DEFAULT_AVG_CHUNK: usize = 32768; // 32 KB
pub const DEFAULT_MAX_CHUNK: usize = 65536; // 64 KB

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DedupFileEntry {
    pub path: String,
    pub size: u64,
    pub modified_unix: u64,
    pub mode: u32,
    pub is_dir: bool,
    pub chunk_hashes: Vec<[u8; 32]>,
    pub file_blake3: [u8; 32],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DedupArchiveMetadata {
    pub version: u32,
    pub created_unix: u64,
    pub uncompressed_size: u64,
    pub deduplicated_raw_size: u64,
    pub total_chunks: usize,
    pub unique_chunks: usize,
    pub total_files: usize,
    pub total_dirs: usize,
    pub files: Vec<DedupFileEntry>,
    pub root_blake3: [u8; 32],
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DedupStats {
    pub original_size: u64,
    pub deduplicated_raw_size: u64,
    pub compressed_archive_size: u64,
    pub total_chunks: usize,
    pub unique_chunks: usize,
    pub total_files: usize,
    pub total_dirs: usize,
    pub dedup_ratio: f64,
    pub space_saved_bytes: u64,
    pub space_saved_percent: f64,
}

pub struct DedupEngine;

impl DedupEngine {
    /// Create a LinRAR Content-Defined Deduplicated Archive (.lrz)
    pub fn create_archive<P: AsRef<Path>>(
        archive_path: P,
        source_paths: &[PathBuf],
        options: &AddOptions,
    ) -> Result<DedupStats, String> {
        let arch_ref = archive_path.as_ref();
        if let Some(parent) = arch_ref.parent()
            && !parent.exists()
        {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        let zstd_level = match options.compression_level {
            CompressionLevel::Store => 1,
            CompressionLevel::Fastest => 1,
            CompressionLevel::Fast => 2,
            CompressionLevel::Normal => 3,
            CompressionLevel::Good => 9,
            CompressionLevel::Best => 19,
        };

        // 1. Collect all files & directories
        let mut file_list: Vec<PathBuf> = Vec::new();
        let mut dir_list: Vec<PathBuf> = Vec::new();

        for src in source_paths {
            if !src.exists() {
                continue;
            }
            if src.is_dir() {
                dir_list.push(src.clone());
                for entry in WalkDir::new(src).into_iter().filter_map(|e| e.ok()) {
                    let p = entry.path().to_path_buf();
                    if p.is_dir() {
                        if p != *src {
                            dir_list.push(p);
                        }
                    } else if p.is_file() {
                        file_list.push(p);
                    }
                }
            } else if src.is_file() {
                file_list.push(src.clone());
            }
        }

        let mut out_file = File::create(arch_ref).map_err(|e| e.to_string())?;

        // Write Magic Header
        out_file
            .write_all(DEDUP_MAGIC_HEADER)
            .map_err(|e| e.to_string())?;
        // Placeholder for unique chunk count (u64)
        out_file
            .write_all(&0u64.to_le_bytes())
            .map_err(|e| e.to_string())?;

        let mut unique_chunk_map: HashMap<[u8; 32], (u64, u32)> = HashMap::new(); // hash -> (offset, compressed_len)
        let mut total_chunks = 0usize;
        let mut total_uncompressed_bytes = 0u64;
        let mut deduplicated_raw_bytes = 0u64;
        let mut entries: Vec<DedupFileEntry> = Vec::new();

        // Register directories
        for d in &dir_list {
            let rel_name = Self::relative_entry_name(d, source_paths);
            let meta = d.metadata().ok();
            let mode = meta
                .as_ref()
                .map(|m| {
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        m.permissions().mode()
                    }
                    #[cfg(not(unix))]
                    {
                        0o755
                    }
                })
                .unwrap_or(0o755);

            let modified = meta
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);

            entries.push(DedupFileEntry {
                path: rel_name,
                size: 0,
                modified_unix: modified,
                mode,
                is_dir: true,
                chunk_hashes: Vec::new(),
                file_blake3: [0u8; 32],
            });
        }

        // Process files with FastCDC chunking + BLAKE3
        for f in &file_list {
            let rel_name = Self::relative_entry_name(f, source_paths);
            let meta = f.metadata().map_err(|e| e.to_string())?;
            let file_size = meta.len();
            total_uncompressed_bytes += file_size;

            let modified = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);

            let mode = {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    meta.permissions().mode()
                }
                #[cfg(not(unix))]
                {
                    0o644
                }
            };

            if file_size == 0 {
                let empty_hash = hash_bytes(b"");
                entries.push(DedupFileEntry {
                    path: rel_name,
                    size: 0,
                    modified_unix: modified,
                    mode,
                    is_dir: false,
                    chunk_hashes: Vec::new(),
                    file_blake3: empty_hash,
                });
                continue;
            }

            // Read file data via streaming Content-Defined Chunking (bounded RAM footprint)
            let file_handle = File::open(f).map_err(|e| e.to_string())?;
            let reader = BufReader::with_capacity(128 * 1024, file_handle);
            let chunker = StreamCDC::new(
                reader,
                DEFAULT_MIN_CHUNK,
                DEFAULT_AVG_CHUNK,
                DEFAULT_MAX_CHUNK,
            );

            let mut file_hasher = blake3::Hasher::new();
            let mut file_chunks = Vec::new();

            for chunk_res in chunker {
                let chunk: ChunkData = chunk_res
                    .map_err(|e| format!("CDC chunking error in '{}': {e}", f.display()))?;
                total_chunks += 1;
                file_hasher.update(&chunk.data);
                let c_hash = hash_bytes(&chunk.data);
                file_chunks.push(c_hash);

                if let std::collections::hash_map::Entry::Vacant(e) = unique_chunk_map.entry(c_hash)
                {
                    // New unique chunk: compress and write to chunk pool
                    deduplicated_raw_bytes += chunk.length as u64;

                    let compressed = zstd::encode_all(&chunk.data[..], zstd_level)
                        .map_err(|e| format!("Zstd chunk compression error: {e}"))?;

                    let current_offset = out_file.stream_position().map_err(|e| e.to_string())?;

                    // Chunk record: [32 bytes Hash][4 bytes raw_len][4 bytes comp_len][compressed data]
                    out_file.write_all(&c_hash).map_err(|e| e.to_string())?;
                    out_file
                        .write_all(&(chunk.length as u32).to_le_bytes())
                        .map_err(|e| e.to_string())?;
                    out_file
                        .write_all(&(compressed.len() as u32).to_le_bytes())
                        .map_err(|e| e.to_string())?;
                    out_file.write_all(&compressed).map_err(|e| e.to_string())?;

                    e.insert((current_offset, compressed.len() as u32));
                }
            }

            let file_hash = *file_hasher.finalize().as_bytes();

            entries.push(DedupFileEntry {
                path: rel_name,
                size: file_size,
                modified_unix: modified,
                mode,
                is_dir: false,
                chunk_hashes: file_chunks,
                file_blake3: file_hash,
            });
        }

        let unique_chunks_count = unique_chunk_map.len();

        // 2. Write metadata table
        let meta_start_offset = out_file.stream_position().map_err(|e| e.to_string())?;

        let all_file_hashes: Vec<[u8; 32]> = entries.iter().map(|e| e.file_blake3).collect();
        let root_blake3 = compute_merkle_root(&all_file_hashes);

        let metadata = DedupArchiveMetadata {
            version: 1,
            created_unix: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            uncompressed_size: total_uncompressed_bytes,
            deduplicated_raw_size: deduplicated_raw_bytes,
            total_chunks,
            unique_chunks: unique_chunks_count,
            total_files: file_list.len(),
            total_dirs: dir_list.len(),
            files: entries,
            root_blake3,
            comment: options.comment.clone(),
        };

        let meta_json = serde_json::to_vec(&metadata).map_err(|e| e.to_string())?;
        let comp_meta = zstd::encode_all(&meta_json[..], zstd_level)
            .map_err(|e| format!("Error compressing archive metadata: {e}"))?;

        out_file
            .write_all(&(comp_meta.len() as u64).to_le_bytes())
            .map_err(|e| e.to_string())?;
        out_file.write_all(&comp_meta).map_err(|e| e.to_string())?;

        // 3. Write Footer
        // [meta_start_offset: u64][root_blake3: 32 bytes][LRDEDEND: 8 bytes]
        out_file
            .write_all(&meta_start_offset.to_le_bytes())
            .map_err(|e| e.to_string())?;
        out_file
            .write_all(&root_blake3)
            .map_err(|e| e.to_string())?;
        out_file
            .write_all(DEDUP_MAGIC_FOOTER)
            .map_err(|e| e.to_string())?;

        // Update header unique chunks count
        out_file
            .seek(SeekFrom::Start(8))
            .map_err(|e| e.to_string())?;
        out_file
            .write_all(&(unique_chunks_count as u64).to_le_bytes())
            .map_err(|e| e.to_string())?;
        out_file.flush().map_err(|e| e.to_string())?;

        let compressed_archive_size = out_file
            .metadata()
            .map(|m| m.len())
            .unwrap_or(meta_start_offset);

        let dedup_ratio = if deduplicated_raw_bytes > 0 {
            total_uncompressed_bytes as f64 / deduplicated_raw_bytes as f64
        } else {
            1.0
        };

        let space_saved = total_uncompressed_bytes.saturating_sub(compressed_archive_size);
        let space_saved_percent = if total_uncompressed_bytes > 0 {
            (space_saved as f64 / total_uncompressed_bytes as f64) * 100.0
        } else {
            0.0
        };

        Ok(DedupStats {
            original_size: total_uncompressed_bytes,
            deduplicated_raw_size: deduplicated_raw_bytes,
            compressed_archive_size,
            total_chunks,
            unique_chunks: unique_chunks_count,
            total_files: file_list.len(),
            total_dirs: dir_list.len(),
            dedup_ratio,
            space_saved_bytes: space_saved,
            space_saved_percent,
        })
    }

    /// Read metadata table from LinRAR deduplicated archive
    pub fn read_metadata<P: AsRef<Path>>(archive_path: P) -> Result<DedupArchiveMetadata, String> {
        let mut file = File::open(archive_path.as_ref()).map_err(|e| e.to_string())?;
        let file_len = file.metadata().map_err(|e| e.to_string())?.len();

        if file_len < 56 {
            return Err("File is too small to be a valid LinRAR Dedup archive".to_string());
        }

        // Check footer: last 8 bytes must be LRDEDEND
        file.seek(SeekFrom::End(-8)).map_err(|e| e.to_string())?;
        let mut footer_magic = [0u8; 8];
        file.read_exact(&mut footer_magic)
            .map_err(|e| e.to_string())?;

        if &footer_magic != DEDUP_MAGIC_FOOTER {
            // Check if at least header magic matches
            file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
            let mut header_magic = [0u8; 8];
            file.read_exact(&mut header_magic)
                .map_err(|e| e.to_string())?;
            if &header_magic == DEDUP_MAGIC_HEADER {
                return Err("Corrupted LinRAR Dedup archive: missing valid footer".to_string());
            }
            return Err("Not a valid LinRAR Dedup archive".to_string());
        }

        // Read footer fields: [meta_offset: u64][root_blake3: 32 bytes][footer_magic: 8 bytes]
        // Offset of meta_offset is file_len - 48
        file.seek(SeekFrom::End(-48)).map_err(|e| e.to_string())?;
        let mut offset_bytes = [0u8; 8];
        file.read_exact(&mut offset_bytes)
            .map_err(|e| e.to_string())?;
        let meta_offset = u64::from_le_bytes(offset_bytes);

        // Seek to meta offset
        file.seek(SeekFrom::Start(meta_offset))
            .map_err(|e| e.to_string())?;
        let mut comp_len_bytes = [0u8; 8];
        file.read_exact(&mut comp_len_bytes)
            .map_err(|e| e.to_string())?;
        let comp_len = u64::from_le_bytes(comp_len_bytes) as usize;

        let mut comp_data = vec![0u8; comp_len];
        file.read_exact(&mut comp_data).map_err(|e| e.to_string())?;

        let decompressed_json = zstd::decode_all(&comp_data[..])
            .map_err(|e| format!("Failed to decompress metadata table: {e}"))?;

        let metadata: DedupArchiveMetadata = serde_json::from_slice(&decompressed_json)
            .map_err(|e| format!("Failed to deserialize archive metadata: {e}"))?;

        Ok(metadata)
    }

    /// Read entries formatted for LinRAR UI and CLI
    pub fn read_entries<P: AsRef<Path>>(
        archive_path: P,
    ) -> Result<(ArchiveInfo, Vec<ArchiveEntry>), String> {
        let meta = Self::read_metadata(archive_path.as_ref())?;
        let packed_size = fs::metadata(archive_path.as_ref())
            .map(|m| m.len())
            .unwrap_or(0);

        let info = ArchiveInfo {
            path: archive_path.as_ref().to_path_buf(),
            archive_type: ArchiveType::Dedup,
            total_files: meta.total_files,
            total_folders: meta.total_dirs,
            unpacked_size: meta.uncompressed_size,
            packed_size,
            is_solid: true, // Content-defined deduplication is an advanced form of cross-file solid deduplication
            is_locked: false,
            has_comment: meta.comment.is_some(),
            comment: meta.comment,
        };

        let mut entries = Vec::with_capacity(meta.files.len());
        for f in meta.files {
            let modified_str = if f.modified_unix > 0 {
                use chrono::DateTime;
                let dt = DateTime::from_timestamp(f.modified_unix as i64, 0).unwrap_or_default();
                dt.format("%Y-%m-%d %H:%M:%S").to_string()
            } else {
                "-".to_string()
            };

            let name = Path::new(&f.path)
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| f.path.clone());

            let crc_str = if f.is_dir {
                "-".to_string()
            } else {
                // First 8 chars of BLAKE3 hex for quick display
                let hex = hash_to_hex(&f.file_blake3);
                hex[..8].to_string()
            };

            let mode_str = crate::types::format_unix_mode(f.mode, f.is_dir);

            entries.push(ArchiveEntry {
                name,
                full_path: f.path,
                size: f.size,
                packed_size: 0, // In dedup archives, chunks are shared globally
                modified: modified_str,
                crc: crc_str,
                is_dir: f.is_dir,
                encrypted: false,
                attributes: mode_str,
            });
        }

        Ok((info, entries))
    }

    /// Extract files from LinRAR deduplicated archive
    pub fn extract<P: AsRef<Path>, D: AsRef<Path>>(
        archive_path: P,
        dest_dir: D,
        selected_files: Option<&[String]>,
    ) -> Result<(), String> {
        let meta = Self::read_metadata(archive_path.as_ref())?;
        let dest = dest_dir.as_ref();
        fs::create_dir_all(dest).map_err(|e| e.to_string())?;

        let mut file = File::open(archive_path.as_ref()).map_err(|e| e.to_string())?;

        // Build chunk offset index from archive chunk pool:
        // [Magic: 8 bytes][unique_count: 8 bytes]
        file.seek(SeekFrom::Start(8)).map_err(|e| e.to_string())?;
        let mut count_bytes = [0u8; 8];
        file.read_exact(&mut count_bytes)
            .map_err(|e| e.to_string())?;
        let unique_count = u64::from_le_bytes(count_bytes) as usize;

        // Map hash -> (file_offset_of_compressed_data, comp_len, raw_len)
        let mut chunk_index: HashMap<[u8; 32], (u64, u32, u32)> =
            HashMap::with_capacity(unique_count);

        let mut current_pos = 16u64;
        for _ in 0..unique_count {
            file.seek(SeekFrom::Start(current_pos))
                .map_err(|e| e.to_string())?;

            let mut hash = [0u8; 32];
            file.read_exact(&mut hash).map_err(|e| e.to_string())?;

            let mut raw_len_bytes = [0u8; 4];
            file.read_exact(&mut raw_len_bytes)
                .map_err(|e| e.to_string())?;
            let raw_len = u32::from_le_bytes(raw_len_bytes);

            let mut comp_len_bytes = [0u8; 4];
            file.read_exact(&mut comp_len_bytes)
                .map_err(|e| e.to_string())?;
            let comp_len = u32::from_le_bytes(comp_len_bytes);

            let data_offset = current_pos + 32 + 4 + 4;
            chunk_index.insert(hash, (data_offset, comp_len, raw_len));

            current_pos = data_offset + comp_len as u64;
        }

        // Extract directories first
        for entry in &meta.files {
            if entry.is_dir {
                if let Some(selected) = selected_files {
                    let matches_sel = selected
                        .iter()
                        .any(|s| entry.path == *s || entry.path.starts_with(&format!("{}/", s)));
                    if !matches_sel {
                        continue;
                    }
                }
                let target = dest.join(&entry.path);
                fs::create_dir_all(&target).map_err(|e| e.to_string())?;
            }
        }

        // Extract files
        for entry in &meta.files {
            if entry.is_dir {
                continue;
            }

            if let Some(selected) = selected_files {
                let matches_sel = selected
                    .iter()
                    .any(|s| entry.path == *s || entry.path.starts_with(&format!("{}/", s)));
                if !matches_sel {
                    continue;
                }
            }

            let target = dest.join(&entry.path);
            if let Some(parent) = target.parent()
                && !parent.exists()
            {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }

            let temp_target = {
                let pid = std::process::id();
                let file_name = target.file_name().unwrap_or_default().to_string_lossy();
                target.with_file_name(format!(".{}.tmp_{}", file_name, pid))
            };

            let f = File::create(&temp_target).map_err(|e| e.to_string())?;
            let mut out = BufWriter::with_capacity(128 * 1024, f);
            let mut hasher = blake3::Hasher::new();

            for chash in &entry.chunk_hashes {
                let &(data_offset, comp_len, _raw_len) = chunk_index
                    .get(chash)
                    .ok_or_else(|| format!("Missing chunk in archive chunk pool: {:?}", chash))?;

                file.seek(SeekFrom::Start(data_offset))
                    .map_err(|e| e.to_string())?;
                let mut comp_buf = vec![0u8; comp_len as usize];
                file.read_exact(&mut comp_buf).map_err(|e| e.to_string())?;

                let decomp = zstd::decode_all(&comp_buf[..])
                    .map_err(|e| format!("Failed to decompress chunk: {e}"))?;

                hasher.update(&decomp);
                out.write_all(&decomp).map_err(|e| e.to_string())?;
            }

            out.flush().map_err(|e| e.to_string())?;
            drop(out);

            // Cryptographic integrity verification against metadata
            let actual_hash = *hasher.finalize().as_bytes();
            if entry.size > 0 && actual_hash != entry.file_blake3 {
                let _ = fs::remove_file(&temp_target);
                return Err(format!(
                    "Cryptographic BLAKE3 integrity check failed for extracted file '{}'",
                    entry.path
                ));
            }

            fs::rename(&temp_target, &target).map_err(|e| e.to_string())?;

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = fs::set_permissions(&target, fs::Permissions::from_mode(entry.mode));
            }
        }

        Ok(())
    }

    /// Test LinRAR deduplicated archive integrity
    pub fn test<P: AsRef<Path>>(archive_path: P) -> Result<bool, String> {
        let meta = Self::read_metadata(archive_path.as_ref())?;
        let mut file = File::open(archive_path.as_ref()).map_err(|e| e.to_string())?;

        // Verify all chunks in chunk pool decompress cleanly and match their BLAKE3 hash
        file.seek(SeekFrom::Start(8)).map_err(|e| e.to_string())?;
        let mut count_bytes = [0u8; 8];
        file.read_exact(&mut count_bytes)
            .map_err(|e| e.to_string())?;
        let unique_count = u64::from_le_bytes(count_bytes) as usize;

        let mut current_pos = 16u64;
        let mut chunk_map: HashMap<[u8; 32], bool> = HashMap::with_capacity(unique_count);

        for _ in 0..unique_count {
            file.seek(SeekFrom::Start(current_pos))
                .map_err(|e| e.to_string())?;

            let mut hash = [0u8; 32];
            file.read_exact(&mut hash).map_err(|e| e.to_string())?;

            let mut raw_len_bytes = [0u8; 4];
            file.read_exact(&mut raw_len_bytes)
                .map_err(|e| e.to_string())?;
            let raw_len = u32::from_le_bytes(raw_len_bytes) as usize;

            let mut comp_len_bytes = [0u8; 4];
            file.read_exact(&mut comp_len_bytes)
                .map_err(|e| e.to_string())?;
            let comp_len = u32::from_le_bytes(comp_len_bytes) as usize;

            let data_offset = current_pos + 32 + 4 + 4;
            file.seek(SeekFrom::Start(data_offset))
                .map_err(|e| e.to_string())?;

            let mut comp_buf = vec![0u8; comp_len];
            file.read_exact(&mut comp_buf).map_err(|e| e.to_string())?;

            let decomp = zstd::decode_all(&comp_buf[..])
                .map_err(|e| format!("Corrupted chunk in archive pool: {e}"))?;

            if decomp.len() != raw_len {
                return Ok(false);
            }

            let actual_chunk_hash = hash_bytes(&decomp);
            if actual_chunk_hash != hash {
                return Ok(false);
            }

            chunk_map.insert(hash, true);
            current_pos = data_offset + comp_len as u64;
        }

        // Verify that all files have their required chunks
        for entry in &meta.files {
            for chash in &entry.chunk_hashes {
                if !chunk_map.contains_key(chash) {
                    return Ok(false);
                }
            }
        }

        // Verify Root Merkle Tree hash
        let all_hashes: Vec<[u8; 32]> = meta.files.iter().map(|e| e.file_blake3).collect();
        let expected_root = compute_merkle_root(&all_hashes);
        if expected_root != meta.root_blake3 {
            return Ok(false);
        }

        Ok(true)
    }

    /// Get detailed deduplication statistics of archive
    pub fn get_stats<P: AsRef<Path>>(archive_path: P) -> Result<DedupStats, String> {
        let meta = Self::read_metadata(archive_path.as_ref())?;
        let packed_size = fs::metadata(archive_path.as_ref())
            .map(|m| m.len())
            .unwrap_or(0);

        let dedup_ratio = if meta.deduplicated_raw_size > 0 {
            meta.uncompressed_size as f64 / meta.deduplicated_raw_size as f64
        } else {
            1.0
        };

        let space_saved = meta.uncompressed_size.saturating_sub(packed_size);
        let space_saved_percent = if meta.uncompressed_size > 0 {
            (space_saved as f64 / meta.uncompressed_size as f64) * 100.0
        } else {
            0.0
        };

        Ok(DedupStats {
            original_size: meta.uncompressed_size,
            deduplicated_raw_size: meta.deduplicated_raw_size,
            compressed_archive_size: packed_size,
            total_chunks: meta.total_chunks,
            unique_chunks: meta.unique_chunks,
            total_files: meta.total_files,
            total_dirs: meta.total_dirs,
            dedup_ratio,
            space_saved_bytes: space_saved,
            space_saved_percent,
        })
    }

    fn relative_entry_name(path: &Path, source_roots: &[PathBuf]) -> String {
        for root in source_roots {
            if path == root {
                return path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
            }
            if let Ok(rel) = path.strip_prefix(root) {
                let base = root.file_name().unwrap_or_default().to_string_lossy();
                let rel_str = rel.to_string_lossy();
                return if rel_str.is_empty() {
                    base.to_string()
                } else {
                    format!("{}/{}", base, rel_str)
                };
            }
        }
        path.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    }
}
