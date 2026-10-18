use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Secure shredding algorithm/standards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShredMethod {
    /// 1 Pass: Overwrite with zero bytes (0x00). Fast and effective for modern SSDs/TRIM.
    QuickZero,
    /// 1 Pass: Overwrite with pseudorandom data.
    Random,
    /// 3 Passes: DoD 5220.22-M compliant (Pass 1: 0x55, Pass 2: 0xAA, Pass 3: Random).
    DoD5220,
    /// 7 Passes: Multi-pattern Gutmann Lite (Alternating bit patterns & random passes).
    GutmannLite,
}

impl ShredMethod {
    pub fn name(&self) -> &'static str {
        match self {
            Self::QuickZero => "Quick Zero (1 pass)",
            Self::Random => "Pseudorandom (1 pass)",
            Self::DoD5220 => "DoD 5220.22-M (3 passes)",
            Self::GutmannLite => "Gutmann Lite (7 passes)",
        }
    }

    #[allow(dead_code)]
    pub fn num_passes(&self) -> usize {
        match self {
            Self::QuickZero => 1,
            Self::Random => 1,
            Self::DoD5220 => 3,
            Self::GutmannLite => 7,
        }
    }
}

/// Internal pass description.
enum PassType {
    Constant(u8),
    Random,
}

/// Fast pseudorandom generator (XorShift128+) seeded from /dev/urandom and high-res time.
struct FastRng {
    s0: u64,
    s1: u64,
}

impl FastRng {
    fn new() -> Self {
        let mut seed0 = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x8543_2910_fedc_ba98);
        let mut seed1 = (std::process::id() as u64) ^ 0x9e37_79b9_7f4a_7c15;

        if let Ok(mut urandom) = File::open("/dev/urandom") {
            let mut buf = [0u8; 16];
            if urandom.read_exact(&mut buf).is_ok() {
                seed0 ^= u64::from_le_bytes(buf[0..8].try_into().unwrap());
                seed1 ^= u64::from_le_bytes(buf[8..16].try_into().unwrap());
            }
        }

        if seed0 == 0 && seed1 == 0 {
            seed0 = 0x5a5a_5a5a_5a5a_5a5a;
            seed1 = 0xa5a5_a5a5_a5a5_a5a5;
        }

        Self {
            s0: seed0,
            s1: seed1,
        }
    }

    #[inline]
    fn next_u64(&mut self) -> u64 {
        let mut s1 = self.s0;
        let s0 = self.s1;
        self.s0 = s0;
        s1 ^= s1 << 23;
        self.s1 = s1 ^ s0 ^ (s1 >> 18) ^ (s0 >> 5);
        self.s1.wrapping_add(s0)
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        let (chunks, rem) = dest.as_chunks_mut::<8>();
        for chunk in chunks {
            let val = self.next_u64();
            chunk.copy_from_slice(&val.to_le_bytes());
        }
        if !rem.is_empty() {
            let val = self.next_u64();
            let bytes = val.to_le_bytes();
            rem.copy_from_slice(&bytes[..rem.len()]);
        }
    }
}

/// Statistics report from a shredding operation.
#[derive(Debug, Default, Clone)]
pub struct ShredReport {
    pub files_shredded: usize,
    pub dirs_removed: usize,
    pub bytes_overwritten: u64,
    pub errors: Vec<String>,
}

/// Engine for secure multi-pass file destruction and data sanitization.
pub struct FileShredder;

impl FileShredder {
    const BUFFER_SIZE: usize = 64 * 1024; // 64 KiB chunks

    /// Securely shreds a single file with the selected algorithm.
    pub fn shred_file<P: AsRef<Path>>(path: P, method: ShredMethod) -> Result<u64, String> {
        let p = path.as_ref();
        if !p.exists() {
            return Err(format!("File does not exist: {}", p.display()));
        }
        if p.is_dir() {
            return Err(format!(
                "Cannot shred directory as a single file: {}",
                p.display()
            ));
        }

        // Ensure permissions permit writing
        if let Ok(meta) = p.metadata() {
            let mut perms = meta.permissions();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if perms.mode() & 0o200 == 0 {
                    perms.set_mode(0o600);
                    let _ = fs::set_permissions(p, perms);
                }
            }
            #[cfg(not(unix))]
            {
                if perms.readonly() {
                    perms.set_readonly(false);
                    let _ = fs::set_permissions(p, perms);
                }
            }
        }

        let file_len = p.metadata().map_err(|e| e.to_string())?.len();
        let mut rng = FastRng::new();

        let passes: Vec<PassType> = match method {
            ShredMethod::QuickZero => vec![PassType::Constant(0x00)],
            ShredMethod::Random => vec![PassType::Random],
            ShredMethod::DoD5220 => vec![
                PassType::Constant(0x55),
                PassType::Constant(0xAA),
                PassType::Random,
            ],
            ShredMethod::GutmannLite => vec![
                PassType::Random,
                PassType::Constant(0x55),
                PassType::Constant(0xAA),
                PassType::Constant(0x92),
                PassType::Constant(0x49),
                PassType::Constant(0x24),
                PassType::Random,
            ],
        };

        if file_len > 0 {
            let mut file = OpenOptions::new()
                .write(true)
                .open(p)
                .map_err(|e| format!("Failed to open file for shredding: {}", e))?;

            let mut buffer = vec![0u8; Self::BUFFER_SIZE];

            for pass in &passes {
                file.seek(SeekFrom::Start(0))
                    .map_err(|e| format!("Seek failed: {}", e))?;

                let mut remaining = file_len;
                while remaining > 0 {
                    let to_write = std::cmp::min(remaining, buffer.len() as u64) as usize;
                    match pass {
                        PassType::Constant(byte) => {
                            buffer[..to_write].fill(*byte);
                        }
                        PassType::Random => {
                            rng.fill_bytes(&mut buffer[..to_write]);
                        }
                    }

                    file.write_all(&buffer[..to_write])
                        .map_err(|e| format!("Write failed during pass: {}", e))?;
                    remaining -= to_write as u64;
                }

                // Flush hardware cache to physical platter / flash memory
                file.sync_all()
                    .map_err(|e| format!("Disk sync failed: {}", e))?;
            }

            // Truncate file to 0 bytes
            file.set_len(0)
                .map_err(|e| format!("Truncate failed: {}", e))?;
            file.sync_all()
                .map_err(|e| format!("Disk sync failed after truncate: {}", e))?;
        }

        // Wipe directory metadata / file name by renaming to a randomized obfuscated name
        let parent = p.parent().unwrap_or(Path::new("."));
        let obfuscated_name = format!(".linrar_wiped_{:016x}", rng.next_u64());
        let obfuscated_path = parent.join(obfuscated_name);

        let final_path = if fs::rename(p, &obfuscated_path).is_ok() {
            obfuscated_path
        } else {
            p.to_path_buf()
        };

        // Remove unlinked file from filesystem
        fs::remove_file(&final_path)
            .map_err(|e| format!("Failed to delete sanitized file: {}", e))?;

        let total_bytes_written = file_len * (passes.len() as u64);
        Ok(total_bytes_written)
    }

    /// Recursively shreds a file or directory tree.
    pub fn shred_path<P: AsRef<Path>>(path: P, method: ShredMethod) -> Result<ShredReport, String> {
        let p = path.as_ref();
        let mut report = ShredReport::default();

        if !p.exists() {
            return Ok(report);
        }

        if p.is_file() {
            match Self::shred_file(p, method) {
                Ok(bytes) => {
                    report.files_shredded += 1;
                    report.bytes_overwritten += bytes;
                }
                Err(e) => {
                    report.errors.push(e);
                }
            }
            return Ok(report);
        }

        // Path is a directory: traverse contents first (bottom-up)
        for entry in walkdir::WalkDir::new(p)
            .contents_first(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let item_path = entry.path();
            if item_path.is_file() {
                match Self::shred_file(item_path, method) {
                    Ok(bytes) => {
                        report.files_shredded += 1;
                        report.bytes_overwritten += bytes;
                    }
                    Err(e) => {
                        report.errors.push(e);
                    }
                }
            } else if item_path.is_dir() {
                if let Err(e) = fs::remove_dir(item_path) {
                    report.errors.push(format!(
                        "Failed to remove directory {}: {}",
                        item_path.display(),
                        e
                    ));
                } else {
                    report.dirs_removed += 1;
                }
            }
        }

        Ok(report)
    }

    /// Shreds multiple paths and aggregates the report.
    pub fn shred_paths(paths: &[PathBuf], method: ShredMethod) -> ShredReport {
        let mut aggregate = ShredReport::default();
        for path in paths {
            match Self::shred_path(path, method) {
                Ok(rep) => {
                    aggregate.files_shredded += rep.files_shredded;
                    aggregate.dirs_removed += rep.dirs_removed;
                    aggregate.bytes_overwritten += rep.bytes_overwritten;
                    aggregate.errors.extend(rep.errors);
                }
                Err(e) => {
                    aggregate.errors.push(e);
                }
            }
        }
        aggregate
    }
}
