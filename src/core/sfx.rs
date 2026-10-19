use crate::core::detector::detect_archive_type;
use crate::types::{ArchiveEntry, ArchiveInfo, ArchiveType, format_unix_mode};
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

pub const SFX_ELF_STUB: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/sfx_elf_stub.bin"));
pub const SFX_FOOTER_MAGIC: u64 = 0x444E45584653524C; // "LRSFXEND" in Little-Endian
pub const SFX_HEADER_MAGIC: &[u8; 8] = b"LINRSFX1";

pub const SFX_FLAG_OVERWRITE: u32 = 1 << 0;
pub const SFX_FLAG_QUIET: u32 = 1 << 1;

pub const SFX_TYPE_TARGZ: u32 = 1;
#[allow(dead_code)]
pub const SFX_TYPE_ZIP: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SfxFormat {
    /// Native Linux ELF 64-bit binary executable (.sfx / .bin)
    Elf,
    /// Universal POSIX shell self-extracting script (.run / .sh)
    Shell,
}

impl SfxFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            SfxFormat::Elf => "sfx",
            SfxFormat::Shell => "run",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            SfxFormat::Elf => "Linux Native Executable (.sfx)",
            SfxFormat::Shell => "Universal Shell Script (.run)",
        }
    }
}

#[derive(Debug, Clone)]
pub struct SfxOptions {
    pub format: SfxFormat,
    pub title: String,
    pub default_dir: String,
    pub run_after: Option<String>,
    pub overwrite: bool,
    pub quiet: bool,
}

impl Default for SfxOptions {
    fn default() -> Self {
        Self {
            format: SfxFormat::Elf,
            title: "LinRAR Archive".to_string(),
            default_dir: ".".to_string(),
            run_after: None,
            overwrite: true,
            quiet: false,
        }
    }
}

pub struct SfxEngine;

impl SfxEngine {
    /// Checks if a given file is a LinRAR Self-Extracting Archive (ELF or Shell script)
    pub fn is_sfx(path: &Path) -> bool {
        if !path.exists() || !path.is_file() {
            return false;
        }

        // 1. Check if ELF SFX via footer magic at EOF - 16
        if let Ok(mut file) = File::open(path)
            && let Ok(meta) = file.metadata()
            && meta.len() >= 32
            && file.seek(SeekFrom::End(-16)).is_ok()
        {
            let mut buf = [0u8; 16];
            if file.read_exact(&mut buf).is_ok() {
                let magic = u64::from_le_bytes(buf[0..8].try_into().unwrap());
                if magic == SFX_FOOTER_MAGIC {
                    return true;
                }
            }
        }

        // 2. Check if Shell SFX via header & marker
        if let Ok(content) = fs::read(path)
            && content.starts_with(b"#!/bin/sh")
            && content
                .windows(b"__LINRAR_SFX_PAYLOAD_BELOW__".len())
                .any(|w| w == b"__LINRAR_SFX_PAYLOAD_BELOW__")
        {
            return true;
        }

        false
    }

    /// Reads embedded metadata from an existing SFX archive
    pub fn read_metadata(path: &Path) -> Result<SfxOptions, String> {
        let mut file = File::open(path).map_err(|e| format!("Cannot open SFX file: {e}"))?;
        let file_len = file
            .metadata()
            .map_err(|e| format!("Failed to read file metadata: {e}"))?
            .len();

        if file_len < 32 {
            return Err("File too small to be a valid SFX archive".to_string());
        }

        // Check footer for ELF SFX
        file.seek(SeekFrom::End(-16))
            .map_err(|e| format!("Seek failed: {e}"))?;
        let mut footer = [0u8; 16];
        file.read_exact(&mut footer)
            .map_err(|e| format!("Read footer failed: {e}"))?;

        let footer_magic = u64::from_le_bytes(footer[0..8].try_into().unwrap());
        let header_offset = u64::from_le_bytes(footer[8..16].try_into().unwrap());

        if footer_magic == SFX_FOOTER_MAGIC && header_offset < file_len {
            file.seek(SeekFrom::Start(header_offset))
                .map_err(|e| format!("Seek to header failed: {e}"))?;

            let mut hdr_bytes = [0u8; 40];
            file.read_exact(&mut hdr_bytes)
                .map_err(|e| format!("Read header failed: {e}"))?;

            if &hdr_bytes[0..8] != SFX_HEADER_MAGIC {
                return Err("Corrupted SFX header magic".to_string());
            }

            let flags = u32::from_le_bytes(hdr_bytes[16..20].try_into().unwrap());
            let title_len = u32::from_le_bytes(hdr_bytes[28..32].try_into().unwrap()) as usize;
            let target_len = u32::from_le_bytes(hdr_bytes[32..36].try_into().unwrap()) as usize;
            let run_len = u32::from_le_bytes(hdr_bytes[36..40].try_into().unwrap()) as usize;

            let mut title_buf = vec![0u8; title_len];
            file.read_exact(&mut title_buf)
                .map_err(|e| format!("Read title failed: {e}"))?;
            let title = String::from_utf8_lossy(&title_buf).to_string();

            let mut target_buf = vec![0u8; target_len];
            file.read_exact(&mut target_buf)
                .map_err(|e| format!("Read target failed: {e}"))?;
            let default_dir = String::from_utf8_lossy(&target_buf).to_string();

            let mut run_buf = vec![0u8; run_len];
            file.read_exact(&mut run_buf)
                .map_err(|e| format!("Read run command failed: {e}"))?;
            let run_after = if run_len > 0 {
                Some(String::from_utf8_lossy(&run_buf).to_string())
            } else {
                None
            };

            return Ok(SfxOptions {
                format: SfxFormat::Elf,
                title,
                default_dir,
                run_after,
                overwrite: (flags & SFX_FLAG_OVERWRITE) != 0,
                quiet: (flags & SFX_FLAG_QUIET) != 0,
            });
        }

        // Fallback for Shell SFX
        file.seek(SeekFrom::Start(0))
            .map_err(|e| format!("Seek failed: {e}"))?;
        let mut script_start = vec![0u8; (file_len.min(4096)) as usize];
        file.read_exact(&mut script_start)
            .map_err(|e| format!("Read failed: {e}"))?;
        let script_str = String::from_utf8_lossy(&script_start);

        if script_str.starts_with("#!/bin/sh") {
            let mut title = "LinRAR SFX Archive".to_string();
            let mut default_dir = ".".to_string();
            let mut run_after = None;
            let mut overwrite = true;

            for line in script_str.lines() {
                if let Some(val) = line.strip_prefix("SFX_TITLE=\"") {
                    title = val.trim_end_matches('"').to_string();
                } else if let Some(val) = line.strip_prefix("SFX_TARGET=\"") {
                    default_dir = val.trim_end_matches('"').to_string();
                } else if let Some(val) = line.strip_prefix("SFX_RUN_AFTER=\"") {
                    let cmd = val.trim_end_matches('"');
                    if !cmd.is_empty() {
                        run_after = Some(cmd.to_string());
                    }
                } else if let Some(val) = line.strip_prefix("SFX_OVERWRITE=") {
                    overwrite = val.trim() == "1";
                }
            }

            return Ok(SfxOptions {
                format: SfxFormat::Shell,
                title,
                default_dir,
                run_after,
                overwrite,
                quiet: false,
            });
        }

        Err("File is not a recognizable LinRAR SFX archive".to_string())
    }

    /// Creates a self-contained SFX archive from a list of files or directories
    pub fn create_sfx(
        output_path: &Path,
        sources: &[PathBuf],
        options: &SfxOptions,
    ) -> Result<(), String> {
        if sources.is_empty() {
            return Err("No source files provided for SFX creation".to_string());
        }

        // 1. Pack sources into a tar.gz payload in memory
        let payload = Self::create_targz_payload(sources)?;

        // 2. Assemble according to chosen SFX format
        match options.format {
            SfxFormat::Elf => Self::assemble_elf_sfx(output_path, &payload, options),
            SfxFormat::Shell => Self::assemble_shell_sfx(output_path, &payload, options),
        }
    }

    /// Converts any existing archive file (zip, rar, 7z, tar.*, iso) into an SFX executable
    pub fn convert_to_sfx(
        archive_path: &Path,
        output_path: &Path,
        options: &SfxOptions,
    ) -> Result<(), String> {
        if !archive_path.exists() {
            return Err(format!(
                "Source archive '{}' does not exist",
                archive_path.display()
            ));
        }

        // Extract source archive into a temporary directory
        let temp_dir = tempfile::tempdir()
            .map_err(|e| format!("Failed to create temporary directory: {e}"))?;

        let arch_type = detect_archive_type(archive_path);
        match arch_type {
            ArchiveType::Zip => {
                crate::core::zip_engine::ZipEngine::extract(
                    archive_path,
                    temp_dir.path(),
                    None,
                    None,
                )?;
            }
            ArchiveType::Tar
            | ArchiveType::TarGz
            | ArchiveType::TarBz2
            | ArchiveType::TarXz
            | ArchiveType::TarZst => {
                crate::core::tar_engine::TarEngine::extract(
                    archive_path,
                    temp_dir.path(),
                    arch_type,
                    None,
                )?;
            }
            ArchiveType::SevenZip => {
                crate::core::sevenzip_engine::SevenZipEngine::extract(
                    archive_path,
                    temp_dir.path(),
                    None,
                    None,
                )?;
            }
            ArchiveType::Iso => {
                crate::core::iso_engine::IsoEngine::extract(archive_path, temp_dir.path(), None)?;
            }
            _ => {
                crate::core::rar_engine::RarEngine::extract(
                    archive_path,
                    temp_dir.path(),
                    None,
                    None,
                )?;
            }
        }

        // Collect all items in temp_dir
        let mut extracted_sources = Vec::new();
        for entry in fs::read_dir(temp_dir.path())
            .map_err(|e| format!("Failed to read extracted directory: {e}"))?
        {
            let entry = entry.map_err(|e| format!("Failed to read dir entry: {e}"))?;
            extracted_sources.push(entry.path());
        }

        if extracted_sources.is_empty() {
            return Err("Extracted source archive is empty".to_string());
        }

        Self::create_sfx(output_path, &extracted_sources, options)
    }

    /// Assembles a Linux ELF Native Self-Extracting binary (.sfx)
    fn assemble_elf_sfx(
        output_path: &Path,
        payload: &[u8],
        options: &SfxOptions,
    ) -> Result<(), String> {
        let mut out =
            File::create(output_path).map_err(|e| format!("Failed to create output file: {e}"))?;

        // 1. Write ELF Stub
        out.write_all(SFX_ELF_STUB)
            .map_err(|e| format!("Failed to write SFX ELF stub: {e}"))?;
        let header_offset = SFX_ELF_STUB.len() as u64;

        // 2. Prepare SfxHeader
        let title_bytes = options.title.as_bytes();
        let target_bytes = options.default_dir.as_bytes();
        let run_bytes = options.run_after.as_deref().unwrap_or("").as_bytes();

        let header_struct_len = 40u32;
        let total_header_size = header_struct_len
            + (title_bytes.len() as u32)
            + (target_bytes.len() as u32)
            + (run_bytes.len() as u32);

        let mut flags = 0u32;
        if options.overwrite {
            flags |= SFX_FLAG_OVERWRITE;
        }
        if options.quiet {
            flags |= SFX_FLAG_QUIET;
        }

        let mut hdr = Vec::new();
        hdr.extend_from_slice(SFX_HEADER_MAGIC); // 8 bytes
        hdr.extend_from_slice(&total_header_size.to_le_bytes()); // 4 bytes
        hdr.extend_from_slice(&SFX_TYPE_TARGZ.to_le_bytes()); // 4 bytes
        hdr.extend_from_slice(&flags.to_le_bytes()); // 4 bytes
        hdr.extend_from_slice(&(payload.len() as u64).to_le_bytes()); // 8 bytes
        hdr.extend_from_slice(&(title_bytes.len() as u32).to_le_bytes()); // 4 bytes
        hdr.extend_from_slice(&(target_bytes.len() as u32).to_le_bytes()); // 4 bytes
        hdr.extend_from_slice(&(run_bytes.len() as u32).to_le_bytes()); // 4 bytes

        hdr.extend_from_slice(title_bytes);
        hdr.extend_from_slice(target_bytes);
        hdr.extend_from_slice(run_bytes);

        out.write_all(&hdr)
            .map_err(|e| format!("Failed to write SFX header: {e}"))?;

        // 3. Write Archive Payload
        out.write_all(payload)
            .map_err(|e| format!("Failed to write SFX payload: {e}"))?;

        // 4. Write SFX Footer (16 bytes: magic + header_offset)
        out.write_all(&SFX_FOOTER_MAGIC.to_le_bytes())
            .map_err(|e| format!("Failed to write footer magic: {e}"))?;
        out.write_all(&header_offset.to_le_bytes())
            .map_err(|e| format!("Failed to write footer offset: {e}"))?;

        out.flush()
            .map_err(|e| format!("Failed to flush output: {e}"))?;

        // 5. Make executable (chmod +x / 0o755)
        let mut perms = out
            .metadata()
            .map_err(|e| format!("Failed to get permissions: {e}"))?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(output_path, perms)
            .map_err(|e| format!("Failed to set executable permission: {e}"))?;

        Ok(())
    }

    /// Assembles a Universal POSIX Shell script Self-Extracting archive (.run / .sh)
    fn assemble_shell_sfx(
        output_path: &Path,
        payload: &[u8],
        options: &SfxOptions,
    ) -> Result<(), String> {
        let title = options.title.replace('"', "\\\"");
        let default_dir = options.default_dir.replace('"', "\\\"");
        let run_after = options
            .run_after
            .as_deref()
            .unwrap_or("")
            .replace('"', "\\\"");
        let overwrite_val = if options.overwrite { "1" } else { "0" };

        let script_header = format!(
            r#"#!/bin/sh
# LinRAR Self-Extracting Archive (Universal POSIX Shell SFX)
# Generated by LinRAR (Modern Universal Archive Manager)

SFX_TITLE="{title}"
SFX_TARGET="{default_dir}"
SFX_RUN_AFTER="{run_after}"
SFX_OVERWRITE={overwrite_val}

print_banner() {{
    echo "==================================================================="
    echo " LinRAR Self-Extracting Archive (Universal POSIX Shell SFX)"
    if [ -n "$SFX_TITLE" ]; then
        echo " Archive: $SFX_TITLE"
    fi
    echo "==================================================================="
}}

print_help() {{
    print_banner
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  -d, --target <DIR>    Extract files to target directory [default: '$SFX_TARGET']"
    echo "  -l, --list            List files in this archive without extracting"
    echo "  -t, --test            Verify archive integrity and test extraction"
    echo "  -y, --overwrite       Overwrite existing files without prompting"
    echo "  -q, --quiet           Silent mode (suppress extraction messages)"
    echo "  -h, --help            Show this help information"
    echo ""
}}

TARGET_DIR="$SFX_TARGET"
MODE_LIST=0
MODE_TEST=0
OPT_QUIET=0

while [ $# -gt 0 ]; do
    case "$1" in
        -h|--help)
            print_help
            exit 0
            ;;
        -l|--list)
            MODE_LIST=1
            shift
            ;;
        -t|--test)
            MODE_TEST=1
            shift
            ;;
        -y|--overwrite)
            SFX_OVERWRITE=1
            shift
            ;;
        -q|--quiet)
            OPT_QUIET=1
            shift
            ;;
        -d|--target|-o)
            if [ -n "$2" ]; then
                TARGET_DIR="$2"
                shift 2
            else
                echo "Error: Option $1 requires an argument" >&2
                exit 1
            fi
            ;;
        *)
            echo "Unknown option: $1" >&2
            print_help
            exit 1
            ;;
    esac
done

# Find where the payload starts
PAYLOAD_LINE=$(awk '/^__LINRAR_SFX_PAYLOAD_BELOW__/ {{print NR + 1; exit 0; }}' "$0")

if [ $MODE_LIST -eq 1 ]; then
    print_banner
    echo "Listing archive contents:"
    echo ""
    tail -n +$PAYLOAD_LINE "$0" | tar -ztvf -
    exit $?
fi

if [ $MODE_TEST -eq 1 ]; then
    print_banner
    echo "Testing archive integrity..."
    tail -n +$PAYLOAD_LINE "$0" | tar -ztf - >/dev/null 2>&1
    RES=$?
    if [ $RES -eq 0 ]; then
        echo "Integrity test PASSED: All files in SFX archive are valid."
    else
        echo "Integrity test FAILED: Archive payload contains corrupted blocks." >&2
    fi
    exit $RES
fi

if [ $OPT_QUIET -eq 0 ]; then
    print_banner
    echo "Extracting archive contents to: $TARGET_DIR"
fi

mkdir -p "$TARGET_DIR" || {{
    echo "Error: Cannot create destination directory '$TARGET_DIR'" >&2
    exit 1
}}

TAR_FLAGS="-zxf"
if [ $OPT_QUIET -eq 0 ]; then
    TAR_FLAGS="-zxvf"
fi

OVERWRITE_FLAG=""
if [ $SFX_OVERWRITE -eq 1 ]; then
    OVERWRITE_FLAG="--overwrite"
fi

tail -n +$PAYLOAD_LINE "$0" | tar $TAR_FLAGS - $OVERWRITE_FLAG -C "$TARGET_DIR"
RES=$?

if [ $RES -ne 0 ]; then
    echo "Error: Extraction encountered errors (exit code $RES)." >&2
    exit $RES
fi

if [ $OPT_QUIET -eq 0 ]; then
    echo ""
    echo "Extraction completed successfully."
fi

if [ -n "$SFX_RUN_AFTER" ]; then
    if [ $OPT_QUIET -eq 0 ]; then
        echo "Executing post-extraction command: $SFX_RUN_AFTER"
    fi
    (cd "$TARGET_DIR" && sh -c "$SFX_RUN_AFTER")
fi

exit 0
__LINRAR_SFX_PAYLOAD_BELOW__
"#
        );

        let mut out =
            File::create(output_path).map_err(|e| format!("Failed to create output file: {e}"))?;

        out.write_all(script_header.as_bytes())
            .map_err(|e| format!("Failed to write shell script header: {e}"))?;
        out.write_all(payload)
            .map_err(|e| format!("Failed to append payload to shell script: {e}"))?;
        out.flush()
            .map_err(|e| format!("Failed to flush output: {e}"))?;

        // chmod +x (0o755)
        let mut perms = out
            .metadata()
            .map_err(|e| format!("Failed to get permissions: {e}"))?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(output_path, perms)
            .map_err(|e| format!("Failed to set executable permissions: {e}"))?;

        Ok(())
    }

    /// Internal helper to compress sources into a tar.gz payload in memory
    fn create_targz_payload(sources: &[PathBuf]) -> Result<Vec<u8>, String> {
        let mut gz_encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
        {
            let mut tar_builder = tar::Builder::new(&mut gz_encoder);

            for src in sources {
                if !src.exists() {
                    return Err(format!("Source path '{}' does not exist", src.display()));
                }

                let file_name = src
                    .file_name()
                    .ok_or_else(|| format!("Invalid source path: {}", src.display()))?;

                if src.is_dir() {
                    tar_builder
                        .append_dir_all(file_name, src)
                        .map_err(|e| format!("Failed to append directory to tar: {e}"))?;
                } else {
                    let mut f = File::open(src)
                        .map_err(|e| format!("Failed to open '{}': {e}", src.display()))?;
                    tar_builder
                        .append_file(file_name, &mut f)
                        .map_err(|e| format!("Failed to append file to tar: {e}"))?;
                }
            }

            tar_builder
                .finish()
                .map_err(|e| format!("Failed to finish tar archive: {e}"))?;
        }

        let compressed = gz_encoder
            .finish()
            .map_err(|e| format!("Failed to finish gzip compression: {e}"))?;

        Ok(compressed)
    }

    /// Extracts payload bytes from an existing SFX file (ELF or Shell)
    pub fn get_payload_bytes(sfx_path: &Path) -> Result<Vec<u8>, String> {
        let mut file = File::open(sfx_path).map_err(|e| format!("Cannot open SFX file: {e}"))?;
        let file_len = file
            .metadata()
            .map_err(|e| format!("Failed to read file metadata: {e}"))?
            .len();

        if file_len < 32 {
            return Err("File too small to be a valid SFX archive".to_string());
        }

        // 1. Try ELF SFX
        if file_len >= 32 && file.seek(SeekFrom::End(-16)).is_ok() {
            let mut footer = [0u8; 16];
            if file.read_exact(&mut footer).is_ok() {
                let footer_magic = u64::from_le_bytes(footer[0..8].try_into().unwrap());
                let header_offset = u64::from_le_bytes(footer[8..16].try_into().unwrap());

                if footer_magic == SFX_FOOTER_MAGIC
                    && header_offset < file_len
                    && file.seek(SeekFrom::Start(header_offset)).is_ok()
                {
                    let mut hdr_bytes = [0u8; 40];
                    if file.read_exact(&mut hdr_bytes).is_ok()
                        && &hdr_bytes[0..8] == SFX_HEADER_MAGIC
                    {
                        let total_header_size =
                            u32::from_le_bytes(hdr_bytes[8..12].try_into().unwrap()) as u64;
                        let payload_len = u64::from_le_bytes(hdr_bytes[20..28].try_into().unwrap());

                        let payload_offset = header_offset + total_header_size;
                        file.seek(SeekFrom::Start(payload_offset))
                            .map_err(|e| format!("Seek failed: {e}"))?;
                        let mut payload = vec![0u8; payload_len as usize];
                        file.read_exact(&mut payload)
                            .map_err(|e| format!("Read payload failed: {e}"))?;
                        return Ok(payload);
                    }
                }
            }
        }

        // 2. Try Shell SFX
        let full_data = fs::read(sfx_path).map_err(|e| format!("Failed to read SFX file: {e}"))?;
        let marker = b"__LINRAR_SFX_PAYLOAD_BELOW__\n";
        if let Some(pos) = full_data.windows(marker.len()).position(|w| w == marker) {
            let payload_start = pos + marker.len();
            return Ok(full_data[payload_start..].to_vec());
        }

        Err("Could not find valid SFX payload in file".to_string())
    }

    /// Reads archive entries from an SFX file for listing or GUI exploration
    pub fn read_entries(
        sfx_path: &Path,
    ) -> Result<(crate::types::ArchiveInfo, Vec<crate::types::ArchiveEntry>), String> {
        let payload = Self::get_payload_bytes(sfx_path)?;
        let gz_decoder = flate2::read::GzDecoder::new(&payload[..]);
        let mut tar_archive = tar::Archive::new(gz_decoder);

        let mut entries = Vec::new();
        let mut total_unpacked = 0u64;

        for entry in tar_archive
            .entries()
            .map_err(|e| format!("Failed to read tar entries: {e}"))?
        {
            let entry = entry.map_err(|e| format!("Corrupted tar entry: {e}"))?;
            let path_cow = entry
                .path()
                .map_err(|e| format!("Invalid path in tar: {e}"))?;
            let full_path = path_cow.to_string_lossy().replace('\\', "/");
            let name = full_path
                .trim_end_matches('/')
                .split('/')
                .rfind(|s| !s.is_empty())
                .unwrap_or(&full_path)
                .to_string();

            let size = entry.size();
            let packed_size = size;
            total_unpacked += size;

            let mtime_sec = entry.header().mtime().unwrap_or(0);
            let modified = chrono::DateTime::from_timestamp(mtime_sec as i64, 0)
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| "-".to_string());

            let is_dir = entry.header().entry_type().is_dir();
            let mode = entry.header().mode().unwrap_or(0);
            let attributes = format_unix_mode(mode, is_dir);

            entries.push(ArchiveEntry {
                name,
                full_path,
                size,
                packed_size,
                modified,
                crc: "-".to_string(),
                is_dir,
                encrypted: false,
                attributes,
            });
        }

        let meta = sfx_path.metadata().ok();
        let file_size = meta.map(|m| m.len()).unwrap_or(payload.len() as u64);
        let sfx_meta = Self::read_metadata(sfx_path).ok();
        let comment = sfx_meta.as_ref().map(|o| o.title.clone());
        let has_comment = comment.is_some();

        let info = ArchiveInfo {
            path: sfx_path.to_path_buf(),
            archive_type: ArchiveType::Sfx,
            total_files: entries.iter().filter(|e| !e.is_dir).count(),
            total_folders: entries.iter().filter(|e| e.is_dir).count(),
            unpacked_size: total_unpacked,
            packed_size: file_size,
            is_solid: true,
            is_locked: false,
            has_comment,
            comment,
        };

        Ok((info, entries))
    }

    /// Extracts files from an SFX file into the destination directory
    pub fn extract(
        sfx_path: &Path,
        dest_dir: &Path,
        selected_files: Option<&[String]>,
    ) -> Result<(), String> {
        let payload = Self::get_payload_bytes(sfx_path)?;
        let gz_decoder = flate2::read::GzDecoder::new(&payload[..]);
        let mut tar_archive = tar::Archive::new(gz_decoder);
        tar_archive.set_preserve_permissions(true);
        tar_archive.set_preserve_mtime(true);

        fs::create_dir_all(dest_dir).map_err(|e| {
            format!(
                "Failed to create destination dir '{}': {e}",
                dest_dir.display()
            )
        })?;

        for entry in tar_archive
            .entries()
            .map_err(|e| format!("Failed to read tar archive entries: {e}"))?
        {
            let mut entry = entry.map_err(|e| format!("Corrupted tar entry: {e}"))?;
            let path_cow = entry.path().map_err(|e| format!("Invalid tar path: {e}"))?;
            let path_str = path_cow.to_string_lossy().replace('\\', "/");

            if let Some(selected) = selected_files {
                let is_match = selected.iter().any(|sel| {
                    let s = sel.replace('\\', "/");
                    path_str == s || path_str.starts_with(&format!("{s}/"))
                });
                if !is_match {
                    continue;
                }
            }

            // Path traversal defense
            let out_file_path = dest_dir.join(&*path_cow);
            if !out_file_path.starts_with(dest_dir) {
                return Err(format!("Malicious path traversal detected: {}", path_str));
            }

            if entry.header().entry_type().is_dir() {
                fs::create_dir_all(&out_file_path)
                    .map_err(|e| format!("Failed to create directory: {e}"))?;
            } else {
                if let Some(parent) = out_file_path.parent() {
                    fs::create_dir_all(parent)
                        .map_err(|e| format!("Failed to create parent directory: {e}"))?;
                }
                entry
                    .unpack(&out_file_path)
                    .map_err(|e| format!("Failed to unpack '{}': {e}", out_file_path.display()))?;
            }
        }

        Ok(())
    }
}
