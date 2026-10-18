use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::types::AddOptions;

/// Smart Bypass / High Speed Archiving Engine.
///
/// Detects already-compressed or high-entropy files (e.g. video, audio, modern image,
/// package, or archive files) to bypass CPU-intensive re-compression cycles.
/// This matches the industry benchmark implementations in WinRAR and Bandizip.
pub struct SmartBypass;

impl SmartBypass {
    /// Comprehensive registry of extensions for formats that are already compressed.
    pub const DEFAULT_EXTENSIONS: &'static [&'static str] = &[
        // Archives & Compressed Packages
        "zip", "rar", "7z", "gz", "bz2", "xz", "zst", "lzma", "lzo", "sz", "tgz", "tbz2", "txz",
        "tzst", "cab", "iso", "dmg", "wim", // Video
        "mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v", "3gp", "ts", "vob", "ogv",
        // Audio
        "mp3", "aac", "ogg", "opus", "flac", "m4a", "wma", "alac",
        // Images (Modern compressed formats)
        "jpg", "jpeg", "png", "webp", "gif", "heic", "heif", "avif", "ico", "jxl",
        // Documents (Zipped XML containers & PDF)
        "docx", "xlsx", "pptx", "odt", "ods", "odp", "epub", "pdf",
        // Packages & Installers
        "apk", "aab", "jar", "war", "ear", "deb", "rpm", "crx", "xpi", "flatpak", "snap", "whl",
        // Web Fonts
        "woff", "woff2",
    ];

    /// Returns the default list of incompressible extensions.
    #[allow(dead_code)]
    pub fn default_extensions() -> &'static [&'static str] {
        Self::DEFAULT_EXTENSIONS
    }

    /// Checks if a file extension is in the known incompressible list.
    pub fn is_incompressible_extension(ext: &str) -> bool {
        let ext_lower = ext.trim_start_matches('.').to_ascii_lowercase();
        Self::DEFAULT_EXTENSIONS.contains(&ext_lower.as_str())
    }

    /// Computes the Shannon entropy of a byte slice in bits per byte (0.0 to 8.0).
    ///
    /// Data with entropy >= 7.6 is considered effectively non-compressible
    /// (e.g. compressed media, encrypted payloads, random data).
    pub fn calculate_entropy(data: &[u8]) -> f64 {
        if data.is_empty() {
            return 0.0;
        }

        let mut counts = [0usize; 256];
        for &byte in data {
            counts[byte as usize] += 1;
        }

        let total = data.len() as f64;
        let mut entropy = 0.0;

        for &count in &counts {
            if count > 0 {
                let p = count as f64 / total;
                entropy -= p * p.log2();
            }
        }

        entropy
    }

    /// Reads up to 8192 bytes from a file and calculates its Shannon entropy.
    pub fn estimate_file_entropy<P: AsRef<Path>>(path: P) -> Option<f64> {
        let mut file = File::open(path).ok()?;
        let mut buffer = [0u8; 8192];
        let bytes_read = file.read(&mut buffer).ok()?;
        if bytes_read == 0 {
            return Some(0.0);
        }
        Some(Self::calculate_entropy(&buffer[..bytes_read]))
    }

    /// Determines whether a file path should bypass compression based on extension
    /// or fallback Shannon entropy estimation for unrecognized/extensionless files.
    pub fn should_bypass<P: AsRef<Path>>(path: P) -> bool {
        let p = path.as_ref();

        // 1. Check extension first (fastest, O(1), zero disk I/O)
        if let Some(ext) = p.extension().and_then(|e| e.to_str())
            && Self::is_incompressible_extension(ext)
        {
            return true;
        }

        // 2. Fallback heuristic: For files >= 4KB without known extension, check entropy
        if let Ok(metadata) = p.metadata()
            && metadata.is_file()
            && metadata.len() >= 4096
            && let Some(entropy) = Self::estimate_file_entropy(p)
            && entropy >= 7.6
        {
            return true;
        }

        false
    }

    /// Determines whether a file path should bypass compression according to `AddOptions`.
    pub fn should_bypass_with_options<P: AsRef<Path>>(path: P, options: &AddOptions) -> bool {
        if !options.smart_bypass {
            return false;
        }

        let p = path.as_ref();

        if let Some(custom_exts) = &options.bypass_extensions {
            if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                let ext_lower = ext.trim_start_matches('.').to_ascii_lowercase();
                if custom_exts
                    .iter()
                    .any(|c| c.trim_start_matches('.').eq_ignore_ascii_case(&ext_lower))
                {
                    return true;
                }
            }
            return false;
        }

        Self::should_bypass(p)
    }
}
