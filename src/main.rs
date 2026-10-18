mod types;
mod core;
mod cli;
mod gui;

use std::path::PathBuf;
use std::time::Duration;
use clap::Parser;

use crate::cli::{Cli, Commands};
use crate::core::benchmark::BenchmarkRunner;
use crate::core::converter::ArchiveConverter;
use crate::core::detector::detect_archive_type;
use crate::core::finder::ArchiveFinder;
use crate::core::rar_engine::RarEngine;
use crate::core::repair::ArchiveRepair;
use crate::core::report::{ArchiveReport, ReportFormat};
use crate::core::scanner::VirusScanner;
use crate::core::tar_engine::TarEngine;
use crate::core::zip_engine::ZipEngine;
use crate::gui::LinrarApp;
use crate::types::{AddOptions, ArchiveType};

fn main() -> eframe::Result {
    let args = Cli::parse();

    // Check if a CLI command was requested
    if let Some(cmd) = args.command.clone() {
        handle_cli_command(cmd, &args);
        return Ok(());
    }

    // Otherwise, launch the full graphical desktop application
    launch_gui(args)
}

fn handle_cli_command(cmd: Commands, args: &Cli) {
    let compression_level = args.get_compression_level();
    let password = args.encrypt_header.clone().or_else(|| args.password.clone());
    let encrypt_headers = args.encrypt_header.is_some();
    let volume_size = args.volume_size.clone();
    let is_solid = args.solid;
    let lock_archive = args.lock;

    let comment = if let Some(cmt_path) = &args.comment_file {
        std::fs::read_to_string(cmt_path).ok()
    } else {
        None
    };

    let options = AddOptions {
        compression_level,
        password,
        encrypt_headers,
        volume_size,
        is_solid,
        lock_archive,
        comment,
    };

    match cmd {
        Commands::Add { archive, files } => {
            let arch_path = PathBuf::from(&archive);
            let sources: Vec<PathBuf> = files.into_iter().map(PathBuf::from).collect();
            let arch_type = detect_archive_type(&arch_path);

            println!("LinRAR 0.1.0 (Rust Engine) - Adding to archive: {}", archive);
            let res = match arch_type {
                ArchiveType::Zip => ZipEngine::create(&arch_path, &sources, &options),
                ArchiveType::TarGz => TarEngine::create(&arch_path, &sources, ArchiveType::TarGz, &options),
                ArchiveType::TarBz2 => TarEngine::create(&arch_path, &sources, ArchiveType::TarBz2, &options),
                ArchiveType::Tar => TarEngine::create(&arch_path, &sources, ArchiveType::Tar, &options),
                _ => RarEngine::create(&arch_path, &sources, &options),
            };

            match res {
                Ok(_) => println!("Done. Archive created successfully."),
                Err(e) => {
                    eprintln!("Error creating archive: {}", e);
                    std::process::exit(1);
                }
            }
        }

        Commands::Extract { archive, destination, files } => {
            let arch_path = PathBuf::from(&archive);
            let dest_path = destination.map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
            let selected = if files.is_empty() { None } else { Some(files.as_slice()) };
            let arch_type = detect_archive_type(&arch_path);

            println!("LinRAR 0.1.0 - Extracting archive: {} to {}", archive, dest_path.display());
            let res = match arch_type {
                ArchiveType::Zip => ZipEngine::extract(&arch_path, &dest_path, selected),
                ArchiveType::TarGz => TarEngine::extract(&arch_path, &dest_path, ArchiveType::TarGz, selected),
                ArchiveType::TarBz2 => TarEngine::extract(&arch_path, &dest_path, ArchiveType::TarBz2, selected),
                ArchiveType::Tar => TarEngine::extract(&arch_path, &dest_path, ArchiveType::Tar, selected),
                _ => RarEngine::extract(&arch_path, &dest_path, selected, args.password.as_deref()),
            };

            match res {
                Ok(_) => println!("All files extracted successfully."),
                Err(e) => {
                    eprintln!("Error extracting archive: {}", e);
                    std::process::exit(1);
                }
            }
        }

        Commands::ExtractFlat { archive, destination, files } => {
            let arch_path = PathBuf::from(&archive);
            let dest_path = destination.map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
            let selected = if files.is_empty() { None } else { Some(files.as_slice()) };

            println!("LinRAR 0.1.0 - Extracting archive flat to: {}", dest_path.display());
            if let Err(e) = RarEngine::extract(&arch_path, &dest_path, selected, args.password.as_deref()) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }

        Commands::List { archive } => {
            let arch_path = PathBuf::from(&archive);
            let arch_type = detect_archive_type(&arch_path);

            let res = match arch_type {
                ArchiveType::Zip => ZipEngine::read_entries(&arch_path),
                ArchiveType::TarGz | ArchiveType::TarBz2 | ArchiveType::Tar => TarEngine::read_entries(&arch_path, arch_type),
                _ => RarEngine::read_entries(&arch_path),
            };

            match res {
                Ok((info, entries)) => {
                    println!("Archive: {}", archive);
                    println!("Format:  {}", info.archive_type.name());
                    println!("{:-<78}", "");
                    println!("{:<36} {:>12} {:>12} {:>10}", "Name", "Size", "Packed", "CRC32");
                    println!("{:-<78}", "");
                    for e in entries {
                        println!("{:<36} {:>12} {:>12} {:>10}", e.name, e.size, e.packed_size, e.crc);
                    }
                    println!("{:-<78}", "");
                    println!("Total: {} files, {} bytes uncompressed", info.total_files, info.unpacked_size);
                }
                Err(e) => {
                    eprintln!("Error listing archive: {}", e);
                    std::process::exit(1);
                }
            }
        }

        Commands::Test { archive } => {
            let arch_path = PathBuf::from(&archive);
            let arch_type = detect_archive_type(&arch_path);
            println!("LinRAR 0.1.0 - Testing archive integrity: {}", archive);
            let res = match arch_type {
                ArchiveType::Zip => ZipEngine::test(&arch_path),
                ArchiveType::TarGz | ArchiveType::TarBz2 | ArchiveType::Tar => TarEngine::test(&arch_path, arch_type),
                _ => RarEngine::test(&arch_path),
            };
            match res {
                Ok(true) => println!("OK - Archive is valid, no errors detected."),
                Ok(false) => {
                    eprintln!("FAILED - Archive integrity check failed.");
                    std::process::exit(1);
                }
                Err(e) => {
                    eprintln!("Error testing archive: {}", e);
                    std::process::exit(1);
                }
            }
        }

        Commands::Delete { archive, files } => {
            let arch_path = PathBuf::from(&archive);
            println!("LinRAR 0.1.0 - Deleting from archive: {}", archive);
            if let Err(e) = RarEngine::delete(&arch_path, &files) {
                eprintln!("Error deleting files: {}", e);
                std::process::exit(1);
            } else {
                println!("Files deleted successfully.");
            }
        }

        Commands::Benchmark { time } => {
            println!("LinRAR Hardware Benchmark (Multi-threaded LZ & CRC32)");
            println!("Threads: {}", rayon::current_num_threads());
            println!("Testing for {} seconds...", time);

            let mut runner = BenchmarkRunner::new();
            runner.start();
            std::thread::sleep(Duration::from_secs(time));
            runner.stop();

            println!("\nBenchmark Results:");
            println!("  Throughput:      {:.0} KB/s ({:.2} MB/s)", runner.current_speed_kb_s(), runner.current_speed_kb_s() / 1024.0);
            println!("  Data Processed:  {:.2} MB", runner.total_processed_mb());
            println!("  Elapsed Time:    {:.2} s", runner.elapsed_seconds());
        }

        Commands::Scan { archive } => {
            let arch_path = PathBuf::from(&archive);
            println!("LinRAR 0.1.0 - Scanning archive for viruses & security threats: {}", archive);
            match VirusScanner::scan_archive(&arch_path) {
                Ok(res) => {
                    println!("Engine:  {}", res.scanner_name);
                    println!("Scanned: {} files", res.files_scanned);
                    if res.clean {
                        println!("Status:  CLEAN - No viruses or suspicious threats detected!");
                    } else {
                        println!("Status:  WARNING - Threats detected ({}):", res.threats.len());
                        for t in &res.threats {
                            println!("  [{}] {}: in {}", t.severity, t.threat_name, t.file);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error scanning archive: {}", e);
                    std::process::exit(1);
                }
            }
        }

        Commands::Repair { archive } => {
            let arch_path = PathBuf::from(&archive);
            println!("LinRAR 0.1.0 - Repairing archive: {}", archive);
            match ArchiveRepair::repair(&arch_path) {
                Ok(res) => {
                    if res.success {
                        println!("Done. Repaired archive created at: {}", res.fixed_path.display());
                    } else {
                        println!("Repair completed with warnings or couldn't recover all records.");
                    }
                    if !res.log.trim().is_empty() {
                        println!("\nLog Output:\n{}", res.log.trim());
                    }
                }
                Err(e) => {
                    eprintln!("Error repairing archive: {}", e);
                    std::process::exit(1);
                }
            }
        }

        Commands::Find { pattern, path, archives } => {
            let target_dir = path.map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
            println!("LinRAR 0.1.0 - Searching for '{}' in {} (inside archives: {})...", pattern, target_dir.display(), archives);
            let matches = ArchiveFinder::find_in_directory(&target_dir, &pattern, archives);
            if matches.is_empty() {
                println!("No matching files found.");
            } else {
                println!("Found {} matching entries:", matches.len());
                for m in matches {
                    println!("  [{}] {} ({} bytes) -> {}", m.container, m.file_name, m.size, m.full_path);
                }
            }
        }

        Commands::Convert { archive, target_format } => {
            let arch_path = PathBuf::from(&archive);
            let fmt = match target_format.to_lowercase().as_str() {
                "zip" => ArchiveType::Zip,
                "tar.gz" | "tgz" => ArchiveType::TarGz,
                "tar.bz2" | "tbz2" => ArchiveType::TarBz2,
                "tar" => ArchiveType::Tar,
                "rar" => ArchiveType::Rar5,
                _ => {
                    eprintln!("Unsupported target format '{}'. Choose from: zip, rar, tar, tar.gz, tar.bz2", target_format);
                    std::process::exit(1);
                }
            };

            println!("LinRAR 0.1.0 - Converting archive '{}' to format '{}'...", archive, fmt.name());
            match ArchiveConverter::convert(&arch_path, fmt, &options) {
                Ok(out_path) => println!("Success! Converted archive saved to: {}", out_path.display()),
                Err(e) => {
                    eprintln!("Conversion failed: {}", e);
                    std::process::exit(1);
                }
            }
        }

        Commands::Report { archive, output, format } => {
            let arch_path = PathBuf::from(&archive);
            let rep_format = ReportFormat::from_str(&format);
            println!("LinRAR 0.1.0 - Generating {} report for: {}", if rep_format == ReportFormat::Html { "HTML" } else { "text" }, archive);

            if let Some(out_file) = output {
                match ArchiveReport::save_to_file(&arch_path, &out_file, rep_format) {
                    Ok(_) => println!("Report saved to: {}", out_file),
                    Err(e) => {
                        eprintln!("Error saving report: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                match ArchiveReport::generate(&arch_path, rep_format) {
                    Ok(content) => println!("{}", content),
                    Err(e) => {
                        eprintln!("Error generating report: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        }

        Commands::Rename { archive, old_name, new_name } => {
            let arch_path = PathBuf::from(&archive);
            println!("LinRAR 0.1.0 - Renaming '{}' -> '{}' in archive: {}", old_name, new_name, archive);
            match RarEngine::rename(&arch_path, &old_name, &new_name) {
                Ok(_) => println!("File renamed successfully."),
                Err(e) => {
                    eprintln!("Error renaming file: {}", e);
                    std::process::exit(1);
                }
            }
        }

        Commands::Protect { archive, percent } => {
            let arch_path = PathBuf::from(&archive);
            println!("LinRAR 0.1.0 - Adding recovery record ({}) to archive: {}", percent, archive);
            match RarEngine::protect(&arch_path, &percent) {
                Ok(_) => println!("Recovery record added successfully."),
                Err(e) => {
                    eprintln!("Error adding recovery record: {}", e);
                    std::process::exit(1);
                }
            }
        }

        Commands::Lock { archive } => {
            let arch_path = PathBuf::from(&archive);
            println!("LinRAR 0.1.0 - Locking archive: {}", archive);
            match RarEngine::lock(&arch_path) {
                Ok(_) => println!("Archive locked successfully against modifications."),
                Err(e) => {
                    eprintln!("Error locking archive: {}", e);
                    std::process::exit(1);
                }
            }
        }

        Commands::Comment { archive, comment } => {
            let arch_path = PathBuf::from(&archive);
            let cmt_text = if let Some(txt) = comment {
                txt
            } else if let Some(cmt_path) = &args.comment_file {
                std::fs::read_to_string(cmt_path).unwrap_or_default()
            } else {
                eprintln!("No comment provided. Use -z <file> or provide comment text as argument.");
                std::process::exit(1);
            };

            println!("LinRAR 0.1.0 - Setting comment on archive: {}", archive);
            match RarEngine::set_comment(&arch_path, &cmt_text) {
                Ok(_) => println!("Archive comment updated successfully."),
                Err(e) => {
                    eprintln!("Error updating comment: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }
}

fn launch_gui(args: Cli) -> eframe::Result {
    let initial_path = args.files.first().map(PathBuf::from);

    let icon_data = if let Ok(img) = image::load_from_memory(include_bytes!("../assets/icons/linrar.png")) {
        Some(egui::IconData {
            rgba: img.to_rgba8().into_raw(),
            width: img.width(),
            height: img.height(),
        })
    } else {
        None
    };

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("LinRAR")
        .with_app_id("linrar")
        .with_inner_size([960.0, 620.0])
        .with_min_inner_size([640.0, 400.0]);

    if let Some(icon) = icon_data {
        viewport = viewport.with_icon(icon);
    }

    let native_options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "LinRAR",
        native_options,
        Box::new(|cc| Ok(Box::new(LinrarApp::new(cc, initial_path)))),
    )
}

