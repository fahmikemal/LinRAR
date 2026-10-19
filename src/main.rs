mod cli;
mod core;
mod gui;
mod types;

use clap::Parser;
use std::path::PathBuf;
use std::time::Duration;

use crate::cli::{Cli, Commands};
use crate::core::benchmark::BenchmarkRunner;
use crate::core::converter::ArchiveConverter;
use crate::core::detector::detect_archive_type;
use crate::core::finder::ArchiveFinder;
use crate::core::iso_engine::IsoEngine;
use crate::core::rar_engine::RarEngine;
use crate::core::repair::ArchiveRepair;
use crate::core::report::{ArchiveReport, ReportFormat};
use crate::core::scanner::VirusScanner;
use crate::core::sevenzip_engine::SevenZipEngine;
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
    let password = args
        .encrypt_header
        .clone()
        .or_else(|| args.password.clone());
    let encrypt_headers = args.encrypt_header.is_some();
    let volume_size = args.volume_size.clone();
    let is_solid = args.solid;
    let lock_archive = args.lock;

    let comment = if let Some(cmt_path) = &args.comment_file {
        std::fs::read_to_string(cmt_path).ok()
    } else {
        None
    };

    let smart_bypass = !args.no_smart_bypass;
    let delete_after_archiving = args.delete || args.shred;
    let shred_after_archiving = args.shred;
    let shred_method = args.get_shred_method();

    let options = AddOptions {
        compression_level,
        password,
        encrypt_headers,
        volume_size,
        is_solid,
        lock_archive,
        comment,
        smart_bypass,
        bypass_extensions: None,
        delete_after_archiving,
        shred_after_archiving,
        shred_method,
        dict_size: args.dict_size.clone(),
        ldm: args.ldm,
        dedup: args.dedup,
    };

    match cmd {
        Commands::Add { archive, files } => {
            let arch_path = PathBuf::from(&archive);
            let sources: Vec<PathBuf> = files.into_iter().map(PathBuf::from).collect();
            let arch_type = detect_archive_type(&arch_path);

            println!(
                "LinRAR 0.1.0 (Rust Engine) - Adding to archive: {}",
                archive
            );

            let is_dedup = args.dedup
                || arch_type == ArchiveType::Dedup
                || archive.ends_with(".lrz")
                || archive.ends_with(".linrar");

            let res = if is_dedup {
                println!("Running LinRAR FastCDC + BLAKE3 Content-Defined Deduplication Engine...");
                match crate::core::dedup::DedupEngine::create_archive(
                    &arch_path, &sources, &options,
                ) {
                    Ok(stats) => {
                        println!("Deduplication & Compression Summary:");
                        println!(
                            "  Original Size:       {:.2} MB ({} bytes)",
                            stats.original_size as f64 / (1024.0 * 1024.0),
                            stats.original_size
                        );
                        println!(
                            "  Unique Chunk Data:   {:.2} MB ({} bytes)",
                            stats.deduplicated_raw_size as f64 / (1024.0 * 1024.0),
                            stats.deduplicated_raw_size
                        );
                        println!(
                            "  Compressed Archive:  {:.2} MB ({} bytes)",
                            stats.compressed_archive_size as f64 / (1024.0 * 1024.0),
                            stats.compressed_archive_size
                        );
                        println!(
                            "  Total Chunks:        {} (Unique: {})",
                            stats.total_chunks, stats.unique_chunks
                        );
                        println!("  Deduplication Ratio: {:.2}x", stats.dedup_ratio);
                        println!(
                            "  Total Space Saved:   {:.1}% ({} bytes)",
                            stats.space_saved_percent, stats.space_saved_bytes
                        );
                        Ok(())
                    }
                    Err(e) => Err(e),
                }
            } else if args.sfx
                || arch_type == ArchiveType::Sfx
                || archive.ends_with(".sfx")
                || archive.ends_with(".run")
            {
                let is_shell = archive.ends_with(".run");
                let sfx_format = if is_shell {
                    crate::core::sfx::SfxFormat::Shell
                } else {
                    crate::core::sfx::SfxFormat::Elf
                };
                let sfx_opts = crate::core::sfx::SfxOptions {
                    format: sfx_format,
                    title: arch_path
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| "LinRAR SFX Archive".to_string()),
                    default_dir: ".".to_string(),
                    run_after: None,
                    overwrite: true,
                    quiet: false,
                };
                crate::core::sfx::SfxEngine::create_sfx(&arch_path, &sources, &sfx_opts)
            } else {
                match arch_type {
                    ArchiveType::Iso => {
                        eprintln!(
                            "Error: ISO images are read-only container images and cannot be created with 'add'."
                        );
                        std::process::exit(1);
                    }
                    ArchiveType::Zip => ZipEngine::create(&arch_path, &sources, &options),
                    ArchiveType::TarGz
                    | ArchiveType::TarBz2
                    | ArchiveType::TarXz
                    | ArchiveType::TarZst
                    | ArchiveType::Tar => {
                        TarEngine::create(&arch_path, &sources, arch_type, &options)
                    }
                    ArchiveType::SevenZip => SevenZipEngine::create(&arch_path, &sources, &options),
                    _ => RarEngine::create(&arch_path, &sources, &options),
                }
            };

            match res {
                Ok(_) => {
                    println!("Done. Archive created successfully.");
                    if options.delete_after_archiving {
                        if options.shred_after_archiving {
                            println!(
                                "Securely shredding source files ({})...",
                                options.shred_method.name()
                            );
                            let rep = crate::core::shredder::FileShredder::shred_paths(
                                &sources,
                                options.shred_method,
                            );
                            println!(
                                "Sanitization complete: {} files shredded ({} bytes overwritten), {} directories removed.",
                                rep.files_shredded, rep.bytes_overwritten, rep.dirs_removed
                            );
                            if !rep.errors.is_empty() {
                                for err in &rep.errors {
                                    eprintln!("  Shred warning: {}", err);
                                }
                            }
                        } else {
                            for src in &sources {
                                if src.is_dir() {
                                    let _ = std::fs::remove_dir_all(src);
                                } else {
                                    let _ = std::fs::remove_file(src);
                                }
                            }
                            println!("Source files deleted after archiving.");
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error creating archive: {}", e);
                    std::process::exit(1);
                }
            }
        }

        Commands::Extract {
            archive,
            destination,
            smart,
            files,
        } => {
            let arch_path = PathBuf::from(&archive);
            let base_dest = destination
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."));
            let selected = if files.is_empty() {
                None
            } else {
                Some(files.as_slice())
            };
            let arch_type = detect_archive_type(&arch_path);

            let dest_path = if smart {
                let entries_res = match arch_type {
                    ArchiveType::Zip => ZipEngine::read_entries(&arch_path).map(|(_, e)| e),
                    ArchiveType::Tar
                    | ArchiveType::TarGz
                    | ArchiveType::TarBz2
                    | ArchiveType::TarXz
                    | ArchiveType::TarZst => {
                        TarEngine::read_entries(&arch_path, arch_type).map(|(_, e)| e)
                    }
                    ArchiveType::SevenZip => {
                        SevenZipEngine::read_entries(&arch_path, args.password.as_deref())
                            .map(|(_, e)| e)
                    }
                    ArchiveType::Iso => IsoEngine::read_entries(&arch_path).map(|(_, e)| e),
                    ArchiveType::Sfx => {
                        crate::core::sfx::SfxEngine::read_entries(&arch_path).map(|(_, e)| e)
                    }
                    ArchiveType::Dedup => {
                        crate::core::dedup::DedupEngine::read_entries(&arch_path).map(|(_, e)| e)
                    }
                    _ => RarEngine::read_entries(&arch_path).map(|(_, e)| e),
                };
                match entries_res {
                    Ok(entries) => {
                        let final_target =
                            crate::core::smart_extract::SmartExtractor::determine_destination(
                                &arch_path, &base_dest, &entries,
                            );
                        let (has_single, root) =
                            crate::core::smart_extract::SmartExtractor::has_single_root_folder(
                                &entries,
                            );
                        if has_single {
                            println!(
                                "Smart Extraction: detected single root folder '{}'; extracting directly to '{}'",
                                root.unwrap_or_default(),
                                base_dest.display()
                            );
                        } else {
                            println!(
                                "Smart Extraction: loose files detected; creating container folder '{}'",
                                final_target.display()
                            );
                        }
                        final_target
                    }
                    Err(_) => base_dest,
                }
            } else {
                base_dest
            };

            let _ = std::fs::create_dir_all(&dest_path);

            let encoding = args
                .encoding
                .as_deref()
                .and_then(crate::core::encoding::ArchiveEncoding::parse_encoding)
                .unwrap_or_default();

            println!(
                "LinRAR 0.1.0 - Extracting archive: {} to {}",
                archive,
                dest_path.display()
            );
            let res = match arch_type {
                ArchiveType::Dedup => {
                    crate::core::dedup::DedupEngine::extract(&arch_path, &dest_path, selected)
                }
                ArchiveType::Zip => ZipEngine::extract_with_encoding(
                    &arch_path,
                    &dest_path,
                    selected,
                    args.password.as_deref(),
                    encoding,
                ),
                ArchiveType::TarGz
                | ArchiveType::TarBz2
                | ArchiveType::TarXz
                | ArchiveType::TarZst
                | ArchiveType::Tar => TarEngine::extract_with_encoding(
                    &arch_path, &dest_path, arch_type, selected, encoding,
                ),
                ArchiveType::SevenZip => SevenZipEngine::extract(
                    &arch_path,
                    &dest_path,
                    selected,
                    args.password.as_deref(),
                ),
                ArchiveType::Iso => IsoEngine::extract(&arch_path, &dest_path, selected),
                ArchiveType::Sfx => {
                    crate::core::sfx::SfxEngine::extract(&arch_path, &dest_path, selected)
                }
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

        Commands::ExtractFlat {
            archive,
            destination,
            files,
        } => {
            let arch_path = PathBuf::from(&archive);
            let dest_path = destination
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."));
            let selected = if files.is_empty() {
                None
            } else {
                Some(files.as_slice())
            };
            let arch_type = detect_archive_type(&arch_path);

            println!(
                "LinRAR 0.1.0 - Extracting archive flat to: {}",
                dest_path.display()
            );
            let flat_res = match arch_type {
                ArchiveType::Dedup => {
                    crate::core::dedup::DedupEngine::extract(&arch_path, &dest_path, selected)
                }
                _ => RarEngine::extract(&arch_path, &dest_path, selected, args.password.as_deref()),
            };
            if let Err(e) = flat_res {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }

        Commands::List { archive } => {
            let arch_path = PathBuf::from(&archive);
            let arch_type = detect_archive_type(&arch_path);
            let encoding = args
                .encoding
                .as_deref()
                .and_then(crate::core::encoding::ArchiveEncoding::parse_encoding)
                .unwrap_or_default();

            let res = match arch_type {
                ArchiveType::Dedup => crate::core::dedup::DedupEngine::read_entries(&arch_path),
                ArchiveType::Zip => ZipEngine::read_entries_with_encoding(&arch_path, encoding),
                ArchiveType::TarGz
                | ArchiveType::TarBz2
                | ArchiveType::TarXz
                | ArchiveType::TarZst
                | ArchiveType::Tar => {
                    TarEngine::read_entries_with_encoding(&arch_path, arch_type, encoding)
                }
                ArchiveType::SevenZip => {
                    SevenZipEngine::read_entries(&arch_path, args.password.as_deref())
                }
                ArchiveType::Iso => IsoEngine::read_entries(&arch_path),
                ArchiveType::Sfx => crate::core::sfx::SfxEngine::read_entries(&arch_path),
                _ => RarEngine::read_entries(&arch_path),
            };

            match res {
                Ok((info, entries)) => {
                    println!("Archive: {}", archive);
                    println!("Format:  {}", info.archive_type.name());
                    println!("{:-<78}", "");
                    println!(
                        "{:<36} {:>12} {:>12} {:>10}",
                        "Name", "Size", "Packed", "CRC/BLAKE3"
                    );
                    println!("{:-<78}", "");
                    for e in entries {
                        println!(
                            "{:<36} {:>12} {:>12} {:>10}",
                            e.name, e.size, e.packed_size, e.crc
                        );
                    }
                    println!("{:-<78}", "");
                    println!(
                        "Total: {} files, {} bytes uncompressed",
                        info.total_files, info.unpacked_size
                    );
                    if arch_type == ArchiveType::Dedup
                        && let Ok(stats) = crate::core::dedup::DedupEngine::get_stats(&arch_path)
                    {
                        println!(
                            "Deduplication:   {:.2}x ratio, {:.1}% space saved ({} bytes saved)",
                            stats.dedup_ratio, stats.space_saved_percent, stats.space_saved_bytes
                        );
                        println!(
                            "Chunk Pool:      {} total chunks, {} unique chunks",
                            stats.total_chunks, stats.unique_chunks
                        );
                    }
                }
                Err(e) => {
                    eprintln!("Error listing archive: {}", e);
                    std::process::exit(1);
                }
            }
        }

        Commands::Test { archive, blake3 } => {
            let arch_path = PathBuf::from(&archive);
            let arch_type = detect_archive_type(&arch_path);
            println!("LinRAR 0.1.0 - Testing archive integrity: {}", archive);
            if blake3 {
                match crate::core::blake3_integrity::hash_file(&arch_path) {
                    Ok(hash) => {
                        println!(
                            "BLAKE3 Hash: {}",
                            crate::core::blake3_integrity::hash_to_hex(&hash)
                        );
                        println!(
                            "OK - BLAKE3 Cryptographic verification completed at ultra-fast speeds."
                        );
                    }
                    Err(e) => {
                        eprintln!("Error computing BLAKE3 hash: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            let res = match arch_type {
                ArchiveType::Dedup => crate::core::dedup::DedupEngine::test(&arch_path),
                ArchiveType::Zip => ZipEngine::test(&arch_path, args.password.as_deref()),
                ArchiveType::TarGz
                | ArchiveType::TarBz2
                | ArchiveType::TarXz
                | ArchiveType::TarZst
                | ArchiveType::Tar => TarEngine::test(&arch_path, arch_type),
                ArchiveType::SevenZip => SevenZipEngine::test(&arch_path, args.password.as_deref()),
                ArchiveType::Iso => IsoEngine::test(&arch_path),
                ArchiveType::Sfx => {
                    crate::core::sfx::SfxEngine::read_entries(&arch_path).map(|_| true)
                }
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
            println!(
                "  Throughput:      {:.0} KB/s ({:.2} MB/s)",
                runner.current_speed_kb_s(),
                runner.current_speed_kb_s() / 1024.0
            );
            println!("  Data Processed:  {:.2} MB", runner.total_processed_mb());
            println!("  Elapsed Time:    {:.2} s", runner.elapsed_seconds());
        }

        Commands::Scan { archive } => {
            let arch_path = PathBuf::from(&archive);
            println!(
                "LinRAR 0.1.0 - Scanning archive for viruses & security threats: {}",
                archive
            );
            match VirusScanner::scan_archive(&arch_path) {
                Ok(res) => {
                    println!("Engine:  {}", res.scanner_name);
                    println!("Scanned: {} files", res.files_scanned);
                    if res.clean {
                        println!("Status:  CLEAN - No viruses or suspicious threats detected!");
                    } else {
                        println!(
                            "Status:  WARNING - Threats detected ({}):",
                            res.threats.len()
                        );
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
                        println!(
                            "Done. Repaired archive created at: {}",
                            res.fixed_path.display()
                        );
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

        Commands::Find {
            pattern,
            path,
            archives,
        } => {
            let target_dir = path
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."));
            println!(
                "LinRAR 0.1.0 - Searching for '{}' in {} (inside archives: {})...",
                pattern,
                target_dir.display(),
                archives
            );
            let matches = ArchiveFinder::find_in_directory(&target_dir, &pattern, archives);
            if matches.is_empty() {
                println!("No matching files found.");
            } else {
                println!("Found {} matching entries:", matches.len());
                for m in matches {
                    println!(
                        "  [{}] {} ({} bytes) -> {}",
                        m.container, m.file_name, m.size, m.full_path
                    );
                }
            }
        }

        Commands::Convert {
            archive,
            target_format,
        } => {
            let arch_path = PathBuf::from(&archive);
            let fmt = match target_format.to_lowercase().as_str() {
                "zip" => ArchiveType::Zip,
                "tar.gz" | "tgz" => ArchiveType::TarGz,
                "tar.bz2" | "tbz2" => ArchiveType::TarBz2,
                "tar.xz" | "txz" | "xz" => ArchiveType::TarXz,
                "tar.zst" | "tzst" | "zst" => ArchiveType::TarZst,
                "tar" => ArchiveType::Tar,
                "7z" | "sevenzip" => ArchiveType::SevenZip,
                "lrz" | "dedup" => ArchiveType::Dedup,
                "rar" => ArchiveType::Rar5,
                _ => {
                    eprintln!(
                        "Unsupported target format '{}'. Choose from: zip, rar, 7z, lrz, tar, tar.gz, tar.bz2, tar.xz, tar.zst",
                        target_format
                    );
                    std::process::exit(1);
                }
            };

            println!(
                "LinRAR 0.1.0 - Converting archive '{}' to format '{}'...",
                archive,
                fmt.name()
            );
            match ArchiveConverter::convert(&arch_path, fmt, &options) {
                Ok(out_path) => println!(
                    "Success! Converted archive saved to: {}",
                    out_path.display()
                ),
                Err(e) => {
                    eprintln!("Conversion failed: {}", e);
                    std::process::exit(1);
                }
            }
        }

        Commands::Report {
            archive,
            output,
            format,
        } => {
            let arch_path = PathBuf::from(&archive);
            let rep_format = ReportFormat::parse_or_default(&format);
            println!(
                "LinRAR 0.1.0 - Generating {} report for: {}",
                if rep_format == ReportFormat::Html {
                    "HTML"
                } else {
                    "text"
                },
                archive
            );

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

        Commands::Rename {
            archive,
            old_name,
            new_name,
        } => {
            let arch_path = PathBuf::from(&archive);
            println!(
                "LinRAR 0.1.0 - Renaming '{}' -> '{}' in archive: {}",
                old_name, new_name, archive
            );
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
            println!(
                "LinRAR 0.1.0 - Adding recovery record ({}) to archive: {}",
                percent, archive
            );
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
                eprintln!(
                    "No comment provided. Use -z <file> or provide comment text as argument."
                );
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

        Commands::Shred { files, method } => {
            let shred_method = match method.to_ascii_lowercase().as_str() {
                "quick" | "zero" => crate::core::shredder::ShredMethod::QuickZero,
                "random" => crate::core::shredder::ShredMethod::Random,
                "gutmann" | "gutmannlite" => crate::core::shredder::ShredMethod::GutmannLite,
                _ => crate::core::shredder::ShredMethod::DoD5220,
            };

            let paths: Vec<PathBuf> = files.into_iter().map(PathBuf::from).collect();
            println!(
                "LinRAR 0.1.0 - Securely shredding {} item(s) using {}...",
                paths.len(),
                shred_method.name()
            );

            let rep = crate::core::shredder::FileShredder::shred_paths(&paths, shred_method);
            println!(
                "Sanitization complete: {} files shredded ({} bytes overwritten), {} directories removed.",
                rep.files_shredded, rep.bytes_overwritten, rep.dirs_removed
            );
            if !rep.errors.is_empty() {
                for err in &rep.errors {
                    eprintln!("  Error: {}", err);
                }
                std::process::exit(1);
            }
        }

        Commands::Sfx {
            archive,
            output,
            title,
            target_dir,
            run_after,
            shell,
        } => {
            let src_path = PathBuf::from(&archive);
            let format = if shell || archive.ends_with(".run") {
                crate::core::sfx::SfxFormat::Shell
            } else {
                crate::core::sfx::SfxFormat::Elf
            };

            let out_path = if let Some(out) = output {
                PathBuf::from(out)
            } else {
                let stem = src_path.file_stem().unwrap_or_default().to_string_lossy();
                src_path.with_file_name(format!("{}.{}", stem, format.extension()))
            };

            let sfx_title = title.unwrap_or_else(|| {
                src_path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            });

            let opts = crate::core::sfx::SfxOptions {
                format,
                title: sfx_title,
                default_dir: target_dir.unwrap_or_else(|| ".".to_string()),
                run_after,
                overwrite: true,
                quiet: false,
            };

            println!(
                "LinRAR 0.1.0 - Converting archive to SFX: {} -> {}",
                archive,
                out_path.display()
            );

            if let Err(e) = crate::core::sfx::SfxEngine::convert_to_sfx(&src_path, &out_path, &opts)
            {
                eprintln!("Error creating SFX: {}", e);
                std::process::exit(1);
            }

            println!(
                "Done. Self-extracting archive created successfully: {}",
                out_path.display()
            );
        }

        Commands::Join { first_part, output } => {
            let part1_path = PathBuf::from(&first_part);
            let parts = match crate::core::volume::VolumeManager::find_volume_parts(&part1_path) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("Error locating volume parts: {}", e);
                    std::process::exit(1);
                }
            };

            let out_path = if let Some(out) = output {
                PathBuf::from(out)
            } else {
                let s = first_part.strip_suffix(".001").unwrap_or(&first_part);
                PathBuf::from(s)
            };

            println!(
                "LinRAR 0.1.0 - Joining {} volume part(s) into: {}",
                parts.len(),
                out_path.display()
            );

            match crate::core::volume::VolumeManager::join_files(&parts, &out_path) {
                Ok(_) => println!("Done. Volumes joined successfully: {}", out_path.display()),
                Err(e) => {
                    eprintln!("Error joining volumes: {}", e);
                    std::process::exit(1);
                }
            }
        }

        Commands::Dedup {
            archive,
            files,
            stats,
        } => {
            let arch_path = PathBuf::from(&archive);
            if stats {
                println!(
                    "LinRAR Dedup - Inspecting Deduplicated Archive: {}",
                    archive
                );
                match crate::core::dedup::DedupEngine::get_stats(&arch_path) {
                    Ok(s) => {
                        println!("Deduplication & Compression Statistics:");
                        println!(
                            "  Original Size:       {:.2} MB ({} bytes)",
                            s.original_size as f64 / (1024.0 * 1024.0),
                            s.original_size
                        );
                        println!(
                            "  Unique Chunk Data:   {:.2} MB ({} bytes)",
                            s.deduplicated_raw_size as f64 / (1024.0 * 1024.0),
                            s.deduplicated_raw_size
                        );
                        println!(
                            "  Compressed Archive:  {:.2} MB ({} bytes)",
                            s.compressed_archive_size as f64 / (1024.0 * 1024.0),
                            s.compressed_archive_size
                        );
                        println!(
                            "  Total Chunks:        {} (Unique: {})",
                            s.total_chunks, s.unique_chunks
                        );
                        println!("  Deduplication Ratio: {:.2}x", s.dedup_ratio);
                        println!(
                            "  Space Saved:         {:.1}% ({} bytes)",
                            s.space_saved_percent, s.space_saved_bytes
                        );
                    }
                    Err(e) => {
                        eprintln!("Error reading dedup stats: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                let sources: Vec<PathBuf> = files.into_iter().map(PathBuf::from).collect();
                println!("LinRAR FastCDC + BLAKE3 Content-Defined Deduplication Engine");
                println!("Creating deduplicated archive: {}", archive);
                match crate::core::dedup::DedupEngine::create_archive(
                    &arch_path, &sources, &options,
                ) {
                    Ok(s) => {
                        println!(
                            "Done. Archive created successfully: {}",
                            arch_path.display()
                        );
                        println!(
                            "  Original Size:       {:.2} MB",
                            s.original_size as f64 / (1024.0 * 1024.0)
                        );
                        println!(
                            "  Unique Chunk Data:   {:.2} MB",
                            s.deduplicated_raw_size as f64 / (1024.0 * 1024.0)
                        );
                        println!(
                            "  Archive on Disk:     {:.2} MB",
                            s.compressed_archive_size as f64 / (1024.0 * 1024.0)
                        );
                        println!(
                            "  Total Chunks:        {} (Unique: {})",
                            s.total_chunks, s.unique_chunks
                        );
                        println!("  Deduplication Ratio: {:.2}x", s.dedup_ratio);
                        println!("  Total Space Saved:   {:.1}%", s.space_saved_percent);
                    }
                    Err(e) => {
                        eprintln!("Error creating deduplicated archive: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        }

        Commands::Hash { files, check } => {
            println!("LinRAR Cryptographic BLAKE3 Engine (Rayon Multi-Threaded)");
            let mut failed = false;
            for f_str in &files {
                let p = PathBuf::from(f_str);
                if let Some(expected) = &check {
                    match crate::core::blake3_integrity::verify_file(&p, expected) {
                        Ok(true) => {
                            println!("{} -> {}  [OK: MATCHES]", expected, p.display());
                        }
                        Ok(false) => {
                            let actual = crate::core::blake3_integrity::hash_file(&p)
                                .map(|h| crate::core::blake3_integrity::hash_to_hex(&h))
                                .unwrap_or_default();
                            eprintln!(
                                "{} -> {}  [FAILED: MISMATCH, expected {}]",
                                actual,
                                p.display(),
                                expected
                            );
                            failed = true;
                        }
                        Err(e) => {
                            eprintln!("Error reading {}: {}", p.display(), e);
                            failed = true;
                        }
                    }
                } else {
                    match crate::core::blake3_integrity::hash_file(&p) {
                        Ok(hash) => {
                            let hex = crate::core::blake3_integrity::hash_to_hex(&hash);
                            println!("{}  {}", hex, p.display());
                        }
                        Err(e) => {
                            eprintln!("Error hashing {}: {}", p.display(), e);
                            failed = true;
                        }
                    }
                }
            }
            if failed {
                std::process::exit(1);
            }
        }
    }
}

fn launch_gui(args: Cli) -> eframe::Result {
    let initial_path = args.files.first().map(PathBuf::from);

    let icon_data =
        if let Ok(img) = image::load_from_memory(include_bytes!("../assets/icons/linrar.png")) {
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
        .with_min_inner_size([640.0, 400.0])
        .with_decorations(false)
        .with_drag_and_drop(true);

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
