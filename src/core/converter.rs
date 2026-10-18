use crate::core::detector::detect_archive_type;
use crate::core::rar_engine::RarEngine;
use crate::core::sevenzip_engine::SevenZipEngine;
use crate::core::smart_extract::SmartExtractor;
use crate::core::tar_engine::TarEngine;
use crate::core::zip_engine::ZipEngine;
use crate::types::{AddOptions, ArchiveType};
use std::fs;
use std::path::{Path, PathBuf};

pub struct ArchiveConverter;

impl ArchiveConverter {
    pub fn convert<P: AsRef<Path>>(
        source_path: P,
        target_format: ArchiveType,
        options: &AddOptions,
    ) -> Result<PathBuf, String> {
        let src = source_path.as_ref();
        if !src.exists() {
            return Err("Source archive does not exist".to_string());
        }

        let src_type = detect_archive_type(src);
        if src_type == target_format {
            return Err("Target format is identical to source format".to_string());
        }

        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let count = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let temp_dir = std::env::temp_dir().join(format!(
            "linrar_convert_{}_{}_{}",
            std::process::id(),
            nanos,
            count
        ));
        fs::create_dir_all(&temp_dir).map_err(|e| e.to_string())?;

        // 1. Extract source
        let ext_res = match src_type {
            ArchiveType::Zip => {
                ZipEngine::extract(src, &temp_dir, None, options.password.as_deref())
            }
            ArchiveType::Tar
            | ArchiveType::TarGz
            | ArchiveType::TarBz2
            | ArchiveType::TarXz
            | ArchiveType::TarZst => TarEngine::extract(src, &temp_dir, src_type, None),
            ArchiveType::SevenZip => {
                SevenZipEngine::extract(src, &temp_dir, None, options.password.as_deref())
            }
            _ => RarEngine::extract(src, &temp_dir, None, options.password.as_deref()),
        };

        if let Err(e) = ext_res {
            let _ = fs::remove_dir_all(&temp_dir);
            return Err(format!("Extraction failed during conversion: {}", e));
        }

        // 2. Determine target path
        let file_stem = SmartExtractor::get_clean_archive_stem(src);
        let target_ext = target_format.default_extension();
        let target_path = src
            .parent()
            .unwrap_or(Path::new("."))
            .join(format!("{}.{}", file_stem, target_ext));

        // Collect extracted contents
        let mut extracted_items = Vec::new();
        if let Ok(entries) = fs::read_dir(&temp_dir) {
            for e in entries.filter_map(|e| e.ok()) {
                extracted_items.push(e.path());
            }
        }

        // 3. Compress into target format
        let comp_res = match target_format {
            ArchiveType::Zip => ZipEngine::create(&target_path, &extracted_items, options),
            ArchiveType::Tar
            | ArchiveType::TarGz
            | ArchiveType::TarBz2
            | ArchiveType::TarXz
            | ArchiveType::TarZst => {
                TarEngine::create(&target_path, &extracted_items, target_format, options)
            }
            ArchiveType::SevenZip => {
                SevenZipEngine::create(&target_path, &extracted_items, options)
            }
            _ => RarEngine::create(&target_path, &extracted_items, options),
        };

        let _ = fs::remove_dir_all(&temp_dir);

        match comp_res {
            Ok(_) => Ok(target_path),
            Err(e) => Err(format!("Compression failed during conversion: {}", e)),
        }
    }
}
