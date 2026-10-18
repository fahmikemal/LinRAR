use crate::core::benchmark::BenchmarkRunner;
use crate::core::finder::SearchMatch;
use crate::core::repair::RepairResult;
use crate::core::report::{ArchiveReport, ReportFormat};
use crate::core::scanner::ScanResult;
use crate::types::{ArchiveInfo, ArchiveType, CompressionLevel};
use std::path::{Path, PathBuf};

pub enum DialogState {
    None,
    AddArchive {
        sources: Vec<PathBuf>,
        archive_name: String,
        format: ArchiveType,
        level: CompressionLevel,
        volume_size: String,
        password: String,
        encrypt_headers: bool,
        is_solid: bool,
        is_locked: bool,
        comment: String,
    },
    ExtractArchive {
        archive_path: PathBuf,
        dest_path: String,
        password: String,
        overwrite: bool,
    },
    ArchiveInfo(ArchiveInfo),
    Benchmark {
        runner: BenchmarkRunner,
    },
    TrialReminder,
    About,
    ScanResults(ScanResult),
    FindFiles {
        pattern: String,
        search_dir: PathBuf,
        search_inside_archives: bool,
        results: Vec<SearchMatch>,
        has_searched: bool,
    },
    ConvertArchive {
        source_path: PathBuf,
        target_format: ArchiveType,
    },
    RepairArchive {
        archive_path: PathBuf,
        result: Option<RepairResult>,
    },
    GenerateReport {
        archive_path: PathBuf,
        format: ReportFormat,
        content: String,
    },
    ProtectArchive {
        archive_path: PathBuf,
        percent: String,
    },
    RenameEntry {
        archive_path: PathBuf,
        old_name: String,
        new_name: String,
    },
    Message {
        title: String,
        message: String,
        is_error: bool,
    },
}

impl DialogState {
    pub fn new_add(sources: Vec<PathBuf>, default_dir: &Path) -> Self {
        let first_name = if let Some(first) = sources.first() {
            first
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        } else {
            "archive".to_string()
        };

        let archive_name = default_dir
            .join(format!("{}.rar", first_name))
            .to_string_lossy()
            .to_string();

        DialogState::AddArchive {
            sources,
            archive_name,
            format: ArchiveType::Rar5,
            level: CompressionLevel::Normal,
            volume_size: "".to_string(),
            password: "".to_string(),
            encrypt_headers: false,
            is_solid: false,
            is_locked: false,
            comment: "".to_string(),
        }
    }

    pub fn new_extract(archive_path: PathBuf) -> Self {
        let parent = archive_path
            .parent()
            .unwrap_or(&archive_path)
            .to_string_lossy()
            .to_string();
        DialogState::ExtractArchive {
            archive_path,
            dest_path: parent,
            password: "".to_string(),
            overwrite: true,
        }
    }

    pub fn new_benchmark() -> Self {
        let mut runner = BenchmarkRunner::new();
        runner.start();
        DialogState::Benchmark { runner }
    }

    pub fn new_find(search_dir: PathBuf) -> Self {
        DialogState::FindFiles {
            pattern: "".to_string(),
            search_dir,
            search_inside_archives: true,
            results: Vec::new(),
            has_searched: false,
        }
    }

    pub fn new_convert(source_path: PathBuf) -> Self {
        DialogState::ConvertArchive {
            source_path,
            target_format: ArchiveType::Zip,
        }
    }

    pub fn new_repair(archive_path: PathBuf) -> Self {
        DialogState::RepairArchive {
            archive_path,
            result: None,
        }
    }

    pub fn new_report(archive_path: PathBuf) -> Self {
        let content =
            ArchiveReport::generate(&archive_path, ReportFormat::Text).unwrap_or_default();
        DialogState::GenerateReport {
            archive_path,
            format: ReportFormat::Text,
            content,
        }
    }

    pub fn new_protect(archive_path: PathBuf) -> Self {
        DialogState::ProtectArchive {
            archive_path,
            percent: "3%".to_string(),
        }
    }

    pub fn new_rename(archive_path: PathBuf, old_name: String) -> Self {
        let new_name = old_name.clone();
        DialogState::RenameEntry {
            archive_path,
            old_name,
            new_name,
        }
    }
}
