use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::core::detector::detect_archive_type;
use crate::core::rar_engine::RarEngine;
use crate::core::tar_engine::TarEngine;
use crate::core::zip_engine::ZipEngine;
use crate::types::{ArchiveEntry, ArchiveType};

pub struct SmartExtractor;

impl SmartExtractor {
    /// Extracts a clean base name for an archive, handling compound multi-part extensions
    /// such as `.tar.gz`, `.tar.bz2`, `.part01.rar`, and `.001`.
    pub fn get_clean_archive_stem<P: AsRef<Path>>(archive_path: P) -> String {
        let p = archive_path.as_ref();
        let file_name = p
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let lower = file_name.to_lowercase();

        // 1. Compound multi-part tar extensions
        for ext in &[
            ".tar.gz", ".tar.bz2", ".tar.xz", ".tar.zst", ".tar.lz4", ".tar.sz",
        ] {
            if lower.ends_with(ext) {
                return file_name[..file_name.len() - ext.len()].to_string();
            }
        }

        // 2. Multi-part RAR (.part01.rar, .part1.rar)
        if let Some(pos) = lower.find(".part")
            && lower.ends_with(".rar")
        {
            return file_name[..pos].to_string();
        }

        // 3. Multi-volume chunks (.001, .002)
        if lower.len() > 4
            && lower.chars().rev().take(3).all(|c| c.is_ascii_digit())
            && lower.chars().rev().nth(3) == Some('.')
        {
            let without_num = &file_name[..file_name.len() - 4];
            let without_lower = without_num.to_lowercase();
            for ext in &[".zip", ".rar", ".tar.gz", ".tar"] {
                if without_lower.ends_with(ext) {
                    return without_num[..without_num.len() - ext.len()].to_string();
                }
            }
            return without_num.to_string();
        }

        // 4. Default standard extension
        p.file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    }

    /// Evaluates if all entries inside the archive belong to a single top-level root directory.
    /// Returns `(true, Some(root_folder_name))` if a single root folder encapsulates all files,
    /// or `(false, None)` if files are loose or scattered across multiple root items.
    pub fn has_single_root_folder(entries: &[ArchiveEntry]) -> (bool, Option<String>) {
        if entries.is_empty() {
            return (false, None);
        }

        let mut root_items: HashSet<String> = HashSet::new();

        for entry in entries {
            let normalized = entry.full_path.trim_matches('/').replace('\\', "/");
            if normalized.is_empty() {
                continue;
            }
            let top_level = match normalized.find('/') {
                Some(idx) => &normalized[..idx],
                None => &normalized,
            };
            root_items.insert(top_level.to_string());
        }

        if root_items.len() == 1 {
            let root_name = root_items.into_iter().next().unwrap();
            let is_directory = entries.iter().any(|e| {
                let norm = e.full_path.trim_matches('/').replace('\\', "/");
                (norm == root_name && e.is_dir) || norm.starts_with(&format!("{}/", root_name))
            });

            if is_directory {
                return (true, Some(root_name));
            }
        }

        (false, None)
    }

    /// Determines the optimal extraction destination path:
    /// - If the archive contains a single root folder, extracts directly into `base_dest`
    ///   (avoiding redundant nested folders `folder/folder/...`).
    /// - If the archive contains loose files or multiple root items, extracts into a newly
    ///   created folder `base_dest/archive_stem/` (preventing desktop archive-bombs).
    pub fn determine_destination<P: AsRef<Path>, D: AsRef<Path>>(
        archive_path: P,
        base_dest: D,
        entries: &[ArchiveEntry],
    ) -> PathBuf {
        let p = archive_path.as_ref();
        let dest = base_dest.as_ref();

        let (has_single_root, _) = Self::has_single_root_folder(entries);
        if has_single_root {
            dest.to_path_buf()
        } else {
            let folder_name = Self::get_clean_archive_stem(p);
            dest.join(folder_name)
        }
    }

    /// High-level Smart Extraction API:
    /// Analyzes archive structure and extracts into the optimal target directory.
    /// Returns the resolved target directory path upon success.
    #[allow(dead_code)]
    pub fn extract_smart<P: AsRef<Path>, D: AsRef<Path>>(
        archive_path: P,
        base_dest: D,
        password: Option<&str>,
    ) -> Result<PathBuf, String> {
        let p = archive_path.as_ref();
        let dest = base_dest.as_ref();

        let arch_type = detect_archive_type(p);
        let entries = match arch_type {
            ArchiveType::Zip => ZipEngine::read_entries(p).map(|(_, e)| e)?,
            ArchiveType::Tar | ArchiveType::TarGz | ArchiveType::TarBz2 => {
                TarEngine::read_entries(p, arch_type).map(|(_, e)| e)?
            }
            ArchiveType::Rar5 | ArchiveType::Rar4 => RarEngine::read_entries(p).map(|(_, e)| e)?,
            _ => return Err("Unsupported archive format for smart extraction".to_string()),
        };

        let target_dir = Self::determine_destination(p, dest, &entries);
        fs::create_dir_all(&target_dir).map_err(|e| e.to_string())?;

        match arch_type {
            ArchiveType::Zip => ZipEngine::extract(p, &target_dir, None, password)?,
            ArchiveType::Tar | ArchiveType::TarGz | ArchiveType::TarBz2 => {
                TarEngine::extract(p, &target_dir, arch_type, None)?
            }
            _ => RarEngine::extract(p, &target_dir, None, password)?,
        }

        Ok(target_dir)
    }
}
