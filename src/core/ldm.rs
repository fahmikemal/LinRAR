use std::io;
use std::io::Write;

/// Maximum dictionary size supported by LinRAR (64 GB, matching & surpassing WinRAR 7.0)
pub const MAX_DICT_SIZE_BYTES: u64 = 64 * 1024 * 1024 * 1024;
/// Minimum dictionary size (128 KB)
pub const MIN_DICT_SIZE_BYTES: u64 = 128 * 1024;

/// Configuration for Long-Distance Matching (LDM) and Extreme Compression Dictionaries
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LdmConfig {
    /// Desired dictionary / window size in bytes
    pub dict_size: u64,
    /// Explicitly enable Zstandard Long-Distance Matching
    pub enable_ldm: bool,
    /// Window log for Zstandard (2^window_log bytes, clamped between 10 and 31)
    pub window_log: u32,
    /// LDM Hash Log (hash table size for LDM search, e.g. 20..26)
    pub ldm_hash_log: Option<u32>,
    /// Minimum match length for LDM (default 64)
    pub ldm_min_match: Option<u32>,
}

impl Default for LdmConfig {
    fn default() -> Self {
        Self {
            dict_size: 128 * 1024 * 1024, // 128 MB default for LDM
            enable_ldm: true,
            window_log: 27, // 2^27 = 128 MB
            ldm_hash_log: Some(22),
            ldm_min_match: Some(64),
        }
    }
}

impl LdmConfig {
    /// Create an LdmConfig from a parsed dictionary size string and optional ldm flag
    pub fn from_dict_str(dict_str: Option<&str>, enable_ldm: bool) -> Result<Option<Self>, String> {
        if dict_str.is_none() && !enable_ldm {
            return Ok(None);
        }

        let dict_size = if let Some(s) = dict_str {
            parse_dict_size(s)?
        } else {
            128 * 1024 * 1024 // 128 MB when --ldm is requested without explicit size
        };

        let window_log = dict_size_to_window_log(dict_size);
        let ldm = enable_ldm || dict_size >= 64 * 1024 * 1024; // Auto-enable LDM for >= 64 MB

        let (hash_log, min_match) = if dict_size >= 1024 * 1024 * 1024 {
            // >= 1 GB: aggressive long match settings
            (Some(24), Some(64))
        } else if dict_size >= 64 * 1024 * 1024 {
            (Some(22), Some(64))
        } else {
            (None, None)
        };

        Ok(Some(Self {
            dict_size,
            enable_ldm: ldm,
            window_log,
            ldm_hash_log: hash_log,
            ldm_min_match: min_match,
        }))
    }

    /// Configure a Zstandard write encoder with this LDM configuration
    pub fn apply_to_zstd<W: Write>(
        &self,
        encoder: &mut zstd::stream::write::Encoder<'static, W>,
    ) -> Result<(), io::Error> {
        // Apply window log
        encoder.set_parameter(zstd::zstd_safe::CParameter::WindowLog(self.window_log))?;

        if self.enable_ldm {
            encoder.long_distance_matching(true)?;

            if let Some(h_log) = self.ldm_hash_log {
                encoder.set_parameter(zstd::zstd_safe::CParameter::LdmHashLog(h_log))?;
            }
            if let Some(min_match) = self.ldm_min_match {
                encoder.set_parameter(zstd::zstd_safe::CParameter::LdmMinMatch(min_match))?;
            }
        }

        Ok(())
    }
}

/// Convert byte size to closest Zstandard window_log (clamped to 10..31)
pub fn dict_size_to_window_log(bytes: u64) -> u32 {
    if bytes <= (1 << 10) {
        return 10;
    }
    // 64-bit leading zeros calculation
    let power = 64 - bytes.leading_zeros() - 1;
    // If not exact power of 2, round up if close
    let log = if (1u64 << power) < bytes {
        power + 1
    } else {
        power
    };

    // Clamp between 10 (1 KB) and 31 (2 GB, maximum single-frame window log in Zstd)
    log.clamp(10, 31)
}

/// Parse human-readable dictionary size string (e.g. "64M", "256M", "1G", "2G", "4G", "16G", "64G")
pub fn parse_dict_size(s: &str) -> Result<u64, String> {
    let trimmed = s.trim().to_uppercase();
    if trimmed.is_empty() {
        return Err("Dictionary size cannot be empty".to_string());
    }

    let (num_part, multiplier) = if trimmed.ends_with("GB") || trimmed.ends_with('G') {
        let num_str = trimmed.trim_end_matches("GB").trim_end_matches('G');
        (num_str, 1024u64 * 1024 * 1024)
    } else if trimmed.ends_with("MB") || trimmed.ends_with('M') {
        let num_str = trimmed.trim_end_matches("MB").trim_end_matches('M');
        (num_str, 1024u64 * 1024)
    } else if trimmed.ends_with("KB") || trimmed.ends_with('K') {
        let num_str = trimmed.trim_end_matches("KB").trim_end_matches('K');
        (num_str, 1024u64)
    } else if trimmed.ends_with('B') {
        let num_str = trimmed.trim_end_matches('B');
        (num_str, 1u64)
    } else {
        // Plain integer in bytes or MB default
        if let Ok(bytes) = trimmed.parse::<u64>() {
            return validate_dict_size(bytes);
        }
        return Err(format!(
            "Invalid dictionary size format '{}'. Expected format e.g. '64M', '256M', '1G', '4G', '64G'",
            s
        ));
    };

    let num = num_part
        .trim()
        .parse::<u64>()
        .map_err(|_| format!("Invalid number in dictionary size '{}'", s))?;

    let total_bytes = num
        .checked_mul(multiplier)
        .ok_or_else(|| format!("Dictionary size '{}' overflows 64-bit integer", s))?;

    validate_dict_size(total_bytes)
}

fn validate_dict_size(bytes: u64) -> Result<u64, String> {
    if bytes < MIN_DICT_SIZE_BYTES {
        return Err(format!(
            "Dictionary size {} is below minimum supported {} bytes (128 KB)",
            format_dict_size(bytes),
            MIN_DICT_SIZE_BYTES
        ));
    }
    if bytes > MAX_DICT_SIZE_BYTES {
        return Err(format!(
            "Dictionary size {} exceeds maximum supported {} (64 GB)",
            format_dict_size(bytes),
            format_dict_size(MAX_DICT_SIZE_BYTES)
        ));
    }
    Ok(bytes)
}

/// Format dictionary size in bytes to friendly string (e.g. "128 MB", "1 GB", "64 GB")
pub fn format_dict_size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 * 1024 {
        let gb = bytes as f64 / (1024.0 * 1024.0 * 1024.0);
        if gb.fract() == 0.0 {
            format!("{:.0} GB", gb)
        } else {
            format!("{:.1} GB", gb)
        }
    } else if bytes >= 1024 * 1024 {
        let mb = bytes as f64 / (1024.0 * 1024.0);
        if mb.fract() == 0.0 {
            format!("{:.0} MB", mb)
        } else {
            format!("{:.1} MB", mb)
        }
    } else if bytes >= 1024 {
        format!("{} KB", bytes / 1024)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_dict_sizes() {
        assert_eq!(parse_dict_size("128K").unwrap(), 128 * 1024);
        assert_eq!(parse_dict_size("64M").unwrap(), 64 * 1024 * 1024);
        assert_eq!(parse_dict_size("128MB").unwrap(), 128 * 1024 * 1024);
        assert_eq!(parse_dict_size("1G").unwrap(), 1024 * 1024 * 1024);
        assert_eq!(parse_dict_size("2GB").unwrap(), 2 * 1024 * 1024 * 1024);
        assert_eq!(parse_dict_size("16G").unwrap(), 16 * 1024 * 1024 * 1024);
        assert_eq!(parse_dict_size("64G").unwrap(), 64 * 1024 * 1024 * 1024);

        assert!(parse_dict_size("128G").is_err());
        assert!(parse_dict_size("32K").is_err());
    }

    #[test]
    fn test_dict_size_to_window_log() {
        assert_eq!(dict_size_to_window_log(1024 * 1024), 20); // 1 MB -> 20
        assert_eq!(dict_size_to_window_log(64 * 1024 * 1024), 26); // 64 MB -> 26
        assert_eq!(dict_size_to_window_log(128 * 1024 * 1024), 27); // 128 MB -> 27
        assert_eq!(dict_size_to_window_log(1024 * 1024 * 1024), 30); // 1 GB -> 30
        assert_eq!(dict_size_to_window_log(2 * 1024 * 1024 * 1024), 31); // 2 GB -> 31
        // Clamped at 31 for > 2GB
        assert_eq!(dict_size_to_window_log(64 * 1024 * 1024 * 1024), 31);
    }
}
