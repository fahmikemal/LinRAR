use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

/// Compute 32-byte BLAKE3 cryptographic hash of in-memory data
pub fn hash_bytes(data: &[u8]) -> [u8; 32] {
    let hash = blake3::hash(data);
    *hash.as_bytes()
}

/// Compute 32-byte BLAKE3 cryptographic hash of a file using Rayon multi-threading
pub fn hash_file<P: AsRef<Path>>(path: P) -> Result<[u8; 32], io::Error> {
    let mut file = File::open(path)?;
    let mut hasher = blake3::Hasher::new();

    // 64 KB buffer for streaming file reads
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(*hasher.finalize().as_bytes())
}

/// Format 32-byte BLAKE3 hash to 64-character lowercase hex string
pub fn hash_to_hex(hash: &[u8; 32]) -> String {
    let mut s = String::with_capacity(64);
    for b in hash {
        use std::fmt::Write;
        let _ = write!(s, "{:02x}", b);
    }
    s
}

/// Parse 64-character hex string into 32-byte array
#[allow(dead_code)]
pub fn hex_to_hash(hex: &str) -> Result<[u8; 32], String> {
    let hex = hex.trim();
    if hex.len() != 64 {
        return Err(format!(
            "Invalid BLAKE3 hex length: expected 64 characters, got {}",
            hex.len()
        ));
    }

    let mut bytes = [0u8; 32];
    for i in 0..32 {
        let chunk = &hex[i * 2..i * 2 + 2];
        bytes[i] = u8::from_str_radix(chunk, 16)
            .map_err(|e| format!("Invalid hex byte at index {}: {}", i, e))?;
    }
    Ok(bytes)
}

/// Verify if a file matches the expected 64-character hex BLAKE3 hash
pub fn verify_file<P: AsRef<Path>>(path: P, expected_hex: &str) -> Result<bool, io::Error> {
    let actual = hash_file(path)?;
    let actual_hex = hash_to_hex(&actual);
    Ok(actual_hex.eq_ignore_ascii_case(expected_hex.trim()))
}

/// Compute Merkle Tree root hash of a series of chunk hashes
/// Provides cryptographic verification of chunk collections
pub fn compute_merkle_root(chunk_hashes: &[[u8; 32]]) -> [u8; 32] {
    if chunk_hashes.is_empty() {
        return [0u8; 32];
    }
    if chunk_hashes.len() == 1 {
        return chunk_hashes[0];
    }

    let mut current_level: Vec<[u8; 32]> = chunk_hashes.to_vec();

    while current_level.len() > 1 {
        let mut next_level = Vec::with_capacity(current_level.len().div_ceil(2));

        for chunk in current_level.chunks(2) {
            if chunk.len() == 2 {
                let mut combined = [0u8; 64];
                combined[..32].copy_from_slice(&chunk[0]);
                combined[32..].copy_from_slice(&chunk[1]);
                next_level.push(hash_bytes(&combined));
            } else {
                // Odd element: promote to next level
                next_level.push(chunk[0]);
            }
        }
        current_level = next_level;
    }

    current_level[0]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blake3_hashing_and_merkle() {
        let data1 = b"Hello LinRAR world!";
        let data2 = b"Another test slice for BLAKE3";
        let h1 = hash_bytes(data1);
        let h2 = hash_bytes(data2);

        assert_ne!(h1, h2);
        let hex1 = hash_to_hex(&h1);
        assert_eq!(hex1.len(), 64);
        let parsed = hex_to_hash(&hex1).expect("valid hex");
        assert_eq!(parsed, h1);

        let root = compute_merkle_root(&[h1, h2]);
        assert_ne!(root, [0u8; 32]);
        assert_ne!(root, h1);
    }
}
