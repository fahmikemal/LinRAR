use std::path::Path;
use walkdir::WalkDir;
use crate::core::detector::{detect_archive_type, is_supported_archive};
use crate::core::rar_engine::RarEngine;
use crate::core::tar_engine::TarEngine;
use crate::core::zip_engine::ZipEngine;
use crate::types::ArchiveType;

#[derive(Debug, Clone)]
pub struct SearchMatch {
    pub container: String, // Archive or Folder name
    pub file_name: String,
    pub full_path: String,
    pub size: u64,
}

pub struct ArchiveFinder;

impl ArchiveFinder {
    pub fn find_in_directory<P: AsRef<Path>>(
        dir_path: P,
        pattern: &str,
        search_inside_archives: bool,
    ) -> Vec<SearchMatch> {
        let pattern_lower = pattern.to_lowercase();
        let mut results = Vec::new();

        for entry in WalkDir::new(dir_path.as_ref()).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            // Match regular files/folders
            if name.to_lowercase().contains(&pattern_lower) {
                results.push(SearchMatch {
                    container: "Local Filesystem".to_string(),
                    file_name: name.clone(),
                    full_path: path.to_string_lossy().to_string(),
                    size: entry.metadata().map(|m| m.len()).unwrap_or(0),
                });
            }

            // Search inside archives if enabled
            if search_inside_archives && entry.file_type().is_file() && is_supported_archive(path) {
                let arch_matches = Self::find_in_archive(path, &pattern_lower);
                results.extend(arch_matches);
            }
        }

        results
    }

    pub fn find_in_archive<P: AsRef<Path>>(archive_path: P, pattern_lower: &str) -> Vec<SearchMatch> {
        let path = archive_path.as_ref();
        let arch_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        let arch_type = detect_archive_type(path);

        let entries_res = match arch_type {
            ArchiveType::Zip => ZipEngine::read_entries(path).map(|(_, e)| e),
            ArchiveType::TarGz | ArchiveType::TarBz2 | ArchiveType::Tar => TarEngine::read_entries(path, arch_type).map(|(_, e)| e),
            _ => RarEngine::read_entries(path).map(|(_, e)| e),
        };

        let mut matches = Vec::new();
        if let Ok(entries) = entries_res {
            for e in entries {
                if e.name.to_lowercase().contains(pattern_lower) || e.full_path.to_lowercase().contains(pattern_lower) {
                    matches.push(SearchMatch {
                        container: arch_name.clone(),
                        file_name: e.name,
                        full_path: e.full_path,
                        size: e.size,
                    });
                }
            }
        }

        matches
    }
}
