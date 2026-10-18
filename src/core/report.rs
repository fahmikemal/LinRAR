use std::fs::File;
use std::io::Write;
use std::path::Path;

use crate::core::detector::detect_archive_type;
use crate::core::rar_engine::RarEngine;
use crate::core::tar_engine::TarEngine;
use crate::core::zip_engine::ZipEngine;
use crate::types::{ArchiveEntry, ArchiveInfo, ArchiveType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportFormat {
    Text,
    Html,
}

impl std::str::FromStr for ReportFormat {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "html" | "htm" => ReportFormat::Html,
            _ => ReportFormat::Text,
        })
    }
}

impl ReportFormat {
    pub fn parse_or_default(s: &str) -> Self {
        s.parse().unwrap_or(ReportFormat::Text)
    }
}

pub struct ArchiveReport;

impl ArchiveReport {
    pub fn generate<P: AsRef<Path>>(
        archive_path: P,
        format: ReportFormat,
    ) -> Result<String, String> {
        let path = archive_path.as_ref();
        if !path.exists() {
            return Err("Archive file does not exist".to_string());
        }

        let arch_type = detect_archive_type(path);
        let (info, entries) = match arch_type {
            ArchiveType::Zip => ZipEngine::read_entries(path)?,
            ArchiveType::TarGz | ArchiveType::TarBz2 | ArchiveType::Tar => {
                TarEngine::read_entries(path, arch_type)?
            }
            _ => RarEngine::read_entries(path)?,
        };

        match format {
            ReportFormat::Text => Ok(Self::format_text(&info, &entries)),
            ReportFormat::Html => Ok(Self::format_html(&info, &entries)),
        }
    }

    pub fn save_to_file<P: AsRef<Path>, O: AsRef<Path>>(
        archive_path: P,
        output_path: O,
        format: ReportFormat,
    ) -> Result<(), String> {
        let content = Self::generate(archive_path, format)?;
        let mut file = File::create(output_path.as_ref()).map_err(|e| e.to_string())?;
        file.write_all(content.as_bytes())
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn format_text(info: &ArchiveInfo, entries: &[ArchiveEntry]) -> String {
        let mut out = String::new();
        out.push_str(
            "================================================================================\n",
        );
        out.push_str(
            "                       LinRAR 0.1.0 - Archive Contents Report                    \n",
        );
        out.push_str(
            "================================================================================\n\n",
        );

        out.push_str(&format!("Archive:       {}\n", info.path.display()));
        out.push_str(&format!("Format:        {}\n", info.archive_type.name()));
        out.push_str(&format!("Total Files:   {}\n", info.total_files));
        out.push_str(&format!("Total Folders: {}\n", info.total_folders));
        out.push_str(&format!(
            "Unpacked Size: {} bytes ({:.2} MB)\n",
            info.unpacked_size,
            info.unpacked_size as f64 / 1_048_576.0
        ));
        out.push_str(&format!(
            "Packed Size:   {} bytes ({:.2} MB)\n",
            info.packed_size,
            info.packed_size as f64 / 1_048_576.0
        ));

        let ratio = if info.unpacked_size > 0 {
            (info.packed_size as f64 / info.unpacked_size as f64) * 100.0
        } else {
            100.0
        };
        out.push_str(&format!("Ratio:         {:.1}%\n", ratio));
        out.push_str(&format!(
            "Solid Archive: {}\n",
            if info.is_solid { "Yes" } else { "No" }
        ));
        out.push_str(&format!(
            "Locked:        {}\n",
            if info.is_locked { "Yes" } else { "No" }
        ));

        if let Some(cmt) = &info.comment {
            out.push_str(&format!("Comment:\n{}\n", cmt));
        }

        out.push_str(
            "\n--------------------------------------------------------------------------------\n",
        );
        out.push_str(&format!(
            "{:<40} {:>10} {:>10} {:>6} {:>8}\n",
            "File Path", "Size", "Packed", "Ratio", "CRC32"
        ));
        out.push_str(
            "--------------------------------------------------------------------------------\n",
        );

        for e in entries {
            let r = if e.size > 0 {
                format!("{:.0}%", (e.packed_size as f64 / e.size as f64) * 100.0)
            } else {
                "-".to_string()
            };

            let disp_name = if e.full_path.len() > 39 {
                format!("...{}", &e.full_path[e.full_path.len() - 36..])
            } else {
                e.full_path.clone()
            };

            out.push_str(&format!(
                "{:<40} {:>10} {:>10} {:>6} {:>8}\n",
                disp_name, e.size, e.packed_size, r, e.crc
            ));
        }

        out.push_str(
            "--------------------------------------------------------------------------------\n",
        );
        out.push_str(&format!("Total: {} items listed.\n", entries.len()));
        out
    }

    fn format_html(info: &ArchiveInfo, entries: &[ArchiveEntry]) -> String {
        let ratio = if info.unpacked_size > 0 {
            (info.packed_size as f64 / info.unpacked_size as f64) * 100.0
        } else {
            100.0
        };

        let mut rows = String::new();
        for e in entries {
            let r = if e.size > 0 {
                format!("{:.0}%", (e.packed_size as f64 / e.size as f64) * 100.0)
            } else {
                "-".to_string()
            };

            let row_type = if e.is_dir { "folder" } else { "file" };
            rows.push_str(&format!(
                "<tr class=\"{}\"><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td><code>{}</code></td><td>{}</td></tr>\n",
                row_type,
                htmlescape(&e.full_path),
                e.size,
                e.packed_size,
                r,
                e.crc,
                if e.encrypted { "🔒 Yes" } else { "No" }
            ));
        }

        format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>LinRAR Archive Report - {}</title>
    <style>
        body {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, monospace; margin: 24px; background: #0d1117; color: #c9d1d9; }}
        h1 {{ color: #20c2d6; border-bottom: 2px solid #30363d; padding-bottom: 8px; font-size: 22px; }}
        .summary {{ background: #161b22; border: 1px solid #30363d; border-radius: 8px; padding: 16px; margin-bottom: 20px; }}
        .summary table {{ border-collapse: collapse; width: 100%; }}
        .summary td {{ padding: 6px 12px; }}
        .summary td:first-child {{ font-weight: bold; width: 160px; color: #8b949e; }}
        table.files {{ width: 100%; border-collapse: collapse; margin-top: 10px; font-size: 13px; }}
        table.files th, table.files td {{ border: 1px solid #30363d; padding: 8px 12px; text-align: left; }}
        table.files th {{ background: #21262d; color: #58a6ff; }}
        table.files tr:nth-child(even) {{ background: #161b22; }}
        table.files tr:hover {{ background: #1f242c; }}
        code {{ font-family: monospace; color: #79c0ff; }}
        .footer {{ margin-top: 24px; font-size: 12px; color: #8b949e; text-align: center; }}
    </style>
</head>
<body>
    <h1>📦 LinRAR 0.1.0 - Archive Contents Report</h1>
    <div class="summary">
        <table>
            <tr><td>Archive:</td><td>{}</td></tr>
            <tr><td>Format:</td><td>{}</td></tr>
            <tr><td>Uncompressed:</td><td>{} bytes ({:.2} MB)</td></tr>
            <tr><td>Compressed:</td><td>{} bytes ({:.2} MB)</td></tr>
            <tr><td>Ratio:</td><td>{:.1}%</td></tr>
            <tr><td>Total Objects:</td><td>{} files, {} folders</td></tr>
            <tr><td>Solid Archive:</td><td>{}</td></tr>
            <tr><td>Locked:</td><td>{}</td></tr>
        </table>
    </div>

    <h2>File Listing ({} items)</h2>
    <table class="files">
        <thead>
            <tr>
                <th>Path</th>
                <th>Original Size</th>
                <th>Packed Size</th>
                <th>Ratio</th>
                <th>CRC32</th>
                <th>Encrypted</th>
            </tr>
        </thead>
        <tbody>
            {}
        </tbody>
    </table>
    <div class="footer">Generated by LinRAR 0.1.0 (Rust Engine) &bull; Universal Archive Manager for Linux (MIT License)</div>
</body>
</html>"#,
            htmlescape(&info.path.file_name().unwrap_or_default().to_string_lossy()),
            htmlescape(&info.path.display().to_string()),
            info.archive_type.name(),
            info.unpacked_size,
            info.unpacked_size as f64 / 1_048_576.0,
            info.packed_size,
            info.packed_size as f64 / 1_048_576.0,
            ratio,
            info.total_files,
            info.total_folders,
            if info.is_solid { "Yes" } else { "No" },
            if info.is_locked { "Yes" } else { "No" },
            entries.len(),
            rows
        )
    }
}

fn htmlescape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
