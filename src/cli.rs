use crate::types::CompressionLevel;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug, Clone)]
#[command(
    name = "linrar",
    author = "LinRAR Team",
    version = "0.1.0",
    about = "Modern Universal Archive Manager for Linux in Pure Rust (MIT License)"
)]
pub struct Cli {
    #[arg(
        short = 'g',
        long = "gui",
        help = "Launch LinRAR graphical user interface"
    )]
    pub gui: bool,

    #[arg(
        short = 'm',
        help = "Set compression level (0-store, 1-fastest, 2-fast, 3-normal, 4-good, 5-best)"
    )]
    pub compression_level: Option<u8>,

    #[arg(short = 'p', help = "Set password")]
    pub password: Option<String>,

    #[arg(long = "hp", help = "Encrypt both file data and header")]
    pub encrypt_header: Option<String>,

    #[arg(short = 'v', help = "Create volumes (e.g. 10m, 100m)")]
    pub volume_size: Option<String>,

    #[arg(short = 's', help = "Create solid archive")]
    pub solid: bool,

    #[arg(short = 'k', help = "Lock archive")]
    pub lock: bool,

    #[arg(short = 'z', help = "Read archive comment from file")]
    pub comment_file: Option<PathBuf>,

    #[arg(
        long = "no-smart-bypass",
        help = "Disable Smart Bypass (force compressing already-compressed media files)"
    )]
    pub no_smart_bypass: bool,

    #[arg(long = "delete", help = "Delete files after successful archiving")]
    pub delete: bool,

    #[arg(
        long = "shred",
        help = "Securely shred files with multi-pass sanitization after archiving"
    )]
    pub shred: bool,

    #[arg(
        long = "shred-method",
        default_value = "quick",
        help = "Shredding method (quick, random, dod, gutmann)"
    )]
    pub shred_method: String,

    #[arg(
        long = "encoding",
        help = "Character encoding / code page for filename decoding (e.g. utf-8, shift-jis, gbk, big5, euc-kr, cp1251, cp1252, cp437)"
    )]
    pub encoding: Option<String>,

    #[arg(long = "sfx", help = "Create self-extracting archive (SFX executable)")]
    pub sfx: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,

    #[arg(trailing_var_arg = true)]
    pub files: Vec<String>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    #[command(name = "a", about = "Add files to archive")]
    Add { archive: String, files: Vec<String> },

    #[command(name = "x", about = "Extract files with full paths")]
    Extract {
        archive: String,
        destination: Option<String>,
        #[arg(
            long = "smart",
            help = "Smart extraction: auto-folder loose files, unpack single-root directly"
        )]
        smart: bool,
        files: Vec<String>,
    },

    #[command(
        name = "e",
        about = "Extract files to current directory (ignore paths)"
    )]
    ExtractFlat {
        archive: String,
        destination: Option<String>,
        files: Vec<String>,
    },

    #[command(name = "l", about = "List contents of archive")]
    List { archive: String },

    #[command(name = "t", about = "Test archive files integrity")]
    Test { archive: String },

    #[command(name = "d", about = "Delete files from archive")]
    Delete { archive: String, files: Vec<String> },

    #[command(name = "b", about = "Run multithreaded hardware benchmark")]
    Benchmark {
        #[arg(
            short = 't',
            long = "time",
            default_value = "5",
            help = "Benchmark duration in seconds"
        )]
        time: u64,
    },

    #[command(name = "s", about = "Scan archive for viruses and security threats")]
    Scan { archive: String },

    #[command(name = "r", about = "Repair damaged archive")]
    Repair { archive: String },

    #[command(
        name = "find",
        alias = "f",
        about = "Find files in directory or inside archives"
    )]
    Find {
        pattern: String,
        path: Option<String>,
        #[arg(short, long, help = "Search inside archives")]
        archives: bool,
    },

    #[command(
        name = "convert",
        about = "Convert archive format (e.g. zip, rar, tar.gz)"
    )]
    Convert {
        archive: String,
        target_format: String,
    },

    #[command(
        name = "report",
        about = "Generate archive contents report (text or html)"
    )]
    Report {
        archive: String,
        #[arg(short, long, help = "Output report file path")]
        output: Option<String>,
        #[arg(long, default_value = "text", help = "Report format: text or html")]
        format: String,
    },

    #[command(name = "rn", about = "Rename file inside archive")]
    Rename {
        archive: String,
        old_name: String,
        new_name: String,
    },

    #[command(name = "rr", about = "Add data recovery record to archive")]
    Protect {
        archive: String,
        #[arg(
            default_value = "3%",
            help = "Recovery record percent (e.g. 3%, 5%, 10%)"
        )]
        percent: String,
    },

    #[command(name = "k", about = "Lock archive against further modifications")]
    Lock { archive: String },

    #[command(name = "c", about = "Set or update archive comment")]
    Comment {
        archive: String,
        #[arg(short, long, help = "Comment text or path to comment file")]
        comment: Option<String>,
    },

    #[command(
        name = "shred",
        about = "Securely shred and sanitize files or directories"
    )]
    Shred {
        files: Vec<String>,
        #[arg(
            short = 'm',
            long = "method",
            default_value = "dod",
            help = "Shredding method (quick, random, dod, gutmann)"
        )]
        method: String,
    },

    #[command(
        name = "sfx",
        about = "Convert archive to self-extracting executable or create SFX"
    )]
    Sfx {
        archive: String,
        #[arg(help = "Output SFX executable path [default: <archive>.sfx]")]
        output: Option<String>,
        #[arg(long, help = "SFX archive title/description")]
        title: Option<String>,
        #[arg(long, help = "Default extraction target directory [default: .]")]
        target_dir: Option<String>,
        #[arg(long, help = "Command to run after extraction")]
        run_after: Option<String>,
        #[arg(
            long = "shell",
            help = "Create universal POSIX shell script (.run) instead of ELF executable (.sfx)"
        )]
        shell: bool,
    },
}

impl Cli {
    pub fn get_compression_level(&self) -> CompressionLevel {
        match self.compression_level.unwrap_or(3) {
            0 => CompressionLevel::Store,
            1 => CompressionLevel::Fastest,
            2 => CompressionLevel::Fast,
            4 => CompressionLevel::Good,
            5 => CompressionLevel::Best,
            _ => CompressionLevel::Normal,
        }
    }

    pub fn get_shred_method(&self) -> crate::core::shredder::ShredMethod {
        match self.shred_method.to_ascii_lowercase().as_str() {
            "random" => crate::core::shredder::ShredMethod::Random,
            "dod" | "dod5220" => crate::core::shredder::ShredMethod::DoD5220,
            "gutmann" | "gutmannlite" => crate::core::shredder::ShredMethod::GutmannLite,
            _ => crate::core::shredder::ShredMethod::QuickZero,
        }
    }
}
