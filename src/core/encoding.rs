use serde::{Deserialize, Serialize};

/// Standard CP437 character lookup table for bytes 128..=255 (IBM PC / MS-DOS OEM)
pub const CP437_TABLE: [char; 128] = [
    'Ç', 'ü', 'é', 'â', 'ä', 'à', 'å', 'ç', 'ê', 'ë', 'è', 'ï', 'î', 'ì', 'Ä', 'Å', 'É', 'æ', 'Æ',
    'ô', 'ö', 'ò', 'û', 'ù', 'ÿ', 'Ö', 'Ü', '¢', '£', '¥', '₧', 'ƒ', 'á', 'í', 'ó', 'ú', 'ñ', 'Ñ',
    'ª', 'º', '¿', '⌐', '¬', '½', '¼', '¡', '«', '»', '░', '▒', '▓', '│', '┤', '╡', '╢', '╖', '╕',
    '╣', '║', '╗', '╝', '╜', '╛', '┐', '└', '┴', '┬', '├', '─', '┼', '╞', '╟', '╚', '╔', '╩', '╦',
    '╠', '═', '╬', '╧', '╨', '╤', '╥', '╙', '╘', '╒', '╓', '╫', '╪', '┘', '┌', '█', '▄', '▌', '▐',
    '▀', 'α', 'ß', 'Γ', 'π', 'Σ', 'σ', 'µ', 'τ', 'Φ', 'Θ', 'Ω', 'δ', '∞', 'φ', 'ε', '∩', '≡', '±',
    '≥', '≤', '⌠', '⌡', '÷', '≈', '°', '∙', '·', '√', 'ⁿ', '²', '■', '\u{00A0}',
];

/// Character encoding / Code Page selector for filenames to eliminate Mojibake
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ArchiveEncoding {
    #[default]
    Auto,
    Utf8,
    ShiftJis,
    Gbk,
    Big5,
    EucKr,
    Windows1251,
    Windows1252,
    Windows1250,
    Cp437,
}

impl ArchiveEncoding {
    /// Canonical identifier string
    pub fn id(&self) -> &'static str {
        match self {
            ArchiveEncoding::Auto => "auto",
            ArchiveEncoding::Utf8 => "utf-8",
            ArchiveEncoding::ShiftJis => "shift-jis",
            ArchiveEncoding::Gbk => "gbk",
            ArchiveEncoding::Big5 => "big5",
            ArchiveEncoding::EucKr => "euc-kr",
            ArchiveEncoding::Windows1251 => "windows-1251",
            ArchiveEncoding::Windows1252 => "windows-1252",
            ArchiveEncoding::Windows1250 => "windows-1250",
            ArchiveEncoding::Cp437 => "cp437",
        }
    }

    /// User-facing descriptive name for GUI ComboBox / Menus
    pub fn display_name(&self) -> &'static str {
        match self {
            ArchiveEncoding::Auto => "Auto (Autodetect)",
            ArchiveEncoding::Utf8 => "UTF-8 (Unicode Standard)",
            ArchiveEncoding::ShiftJis => "Shift-JIS / CP932 (Japanese)",
            ArchiveEncoding::Gbk => "GBK / GB18030 (Simplified Chinese)",
            ArchiveEncoding::Big5 => "Big5 / CP950 (Traditional Chinese)",
            ArchiveEncoding::EucKr => "EUC-KR / CP949 (Korean)",
            ArchiveEncoding::Windows1251 => "Windows-1251 (Cyrillic: RU, UA, BG)",
            ArchiveEncoding::Windows1252 => "Windows-1252 (Western European: EN, FR, DE)",
            ArchiveEncoding::Windows1250 => "Windows-1250 (Central European: PL, CZ, HU)",
            ArchiveEncoding::Cp437 => "CP437 (IBM PC / MS-DOS Legacy)",
        }
    }

    /// List of all supported encodings
    pub fn all() -> &'static [ArchiveEncoding] {
        &[
            ArchiveEncoding::Auto,
            ArchiveEncoding::Utf8,
            ArchiveEncoding::ShiftJis,
            ArchiveEncoding::Gbk,
            ArchiveEncoding::Big5,
            ArchiveEncoding::EucKr,
            ArchiveEncoding::Windows1251,
            ArchiveEncoding::Windows1252,
            ArchiveEncoding::Windows1250,
            ArchiveEncoding::Cp437,
        ]
    }

    /// Parses string representation (case-insensitive and alias-tolerant)
    pub fn parse_encoding(s: &str) -> Option<ArchiveEncoding> {
        s.parse().ok()
    }

    /// Decodes raw filename byte slice into a clean UTF-8 string according to the selected encoding
    pub fn decode(&self, bytes: &[u8]) -> String {
        match self {
            ArchiveEncoding::Auto => Self::detect_and_decode(bytes),
            ArchiveEncoding::Utf8 => String::from_utf8_lossy(bytes).to_string(),
            ArchiveEncoding::ShiftJis => encoding_rs::SHIFT_JIS.decode(bytes).0.to_string(),
            ArchiveEncoding::Gbk => encoding_rs::GB18030.decode(bytes).0.to_string(),
            ArchiveEncoding::Big5 => encoding_rs::BIG5.decode(bytes).0.to_string(),
            ArchiveEncoding::EucKr => encoding_rs::EUC_KR.decode(bytes).0.to_string(),
            ArchiveEncoding::Windows1251 => encoding_rs::WINDOWS_1251.decode(bytes).0.to_string(),
            ArchiveEncoding::Windows1252 => encoding_rs::WINDOWS_1252.decode(bytes).0.to_string(),
            ArchiveEncoding::Windows1250 => encoding_rs::WINDOWS_1250.decode(bytes).0.to_string(),
            ArchiveEncoding::Cp437 => Self::decode_cp437(bytes),
        }
    }

    /// Decodes CP437 byte stream
    pub fn decode_cp437(bytes: &[u8]) -> String {
        bytes
            .iter()
            .map(|&b| {
                if b < 128 {
                    b as char
                } else {
                    CP437_TABLE[(b - 128) as usize]
                }
            })
            .collect()
    }

    /// Autodetects encoding for non-UTF8 filename bytes
    pub fn detect_and_decode(bytes: &[u8]) -> String {
        // 1. If valid UTF-8, use directly
        if let Ok(s) = std::str::from_utf8(bytes) {
            return s.to_string();
        }

        // 2. Test candidate encodings without replacement characters
        // Shift-JIS (Common for Japanese archives)
        let (cow, had_errors) = encoding_rs::SHIFT_JIS.decode_without_bom_handling(bytes);
        if !had_errors && !cow.contains('\u{FFFD}') {
            return cow.to_string();
        }

        // GB18030 / GBK (Common for Chinese archives)
        let (cow, had_errors) = encoding_rs::GB18030.decode_without_bom_handling(bytes);
        if !had_errors && !cow.contains('\u{FFFD}') {
            return cow.to_string();
        }

        // EUC-KR (Common for Korean archives)
        let (cow, had_errors) = encoding_rs::EUC_KR.decode_without_bom_handling(bytes);
        if !had_errors && !cow.contains('\u{FFFD}') {
            return cow.to_string();
        }

        // Windows-1251 (Common for Cyrillic / Russian archives)
        // Check if bytes fall within typical Cyrillic range 0xC0..=0xFF
        let cyrillic_count = bytes.iter().filter(|&&b| b >= 0xC0).count();
        if cyrillic_count > 0 && cyrillic_count * 2 >= bytes.len() {
            let (cow, _, _) = encoding_rs::WINDOWS_1251.decode(bytes);
            return cow.to_string();
        }

        // 3. Fall back to Windows-1252 (Western European Latin-1)
        encoding_rs::WINDOWS_1252.decode(bytes).0.to_string()
    }

    /// Helper to decode a path and extract clean name and normalized full path
    pub fn decode_path(&self, raw_bytes: &[u8]) -> (String, String) {
        let full_path = self.decode(raw_bytes);
        let normalized = full_path.replace('\\', "/");
        let name = normalized
            .trim_end_matches('/')
            .split('/')
            .rfind(|s| !s.is_empty())
            .unwrap_or(&normalized)
            .to_string();
        (name, normalized)
    }
}

impl std::str::FromStr for ArchiveEncoding {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let norm = s.trim().to_lowercase().replace(['_', '-'], "");
        match norm.as_str() {
            "auto" | "default" => Ok(ArchiveEncoding::Auto),
            "utf8" | "utf-8" | "unicode" => Ok(ArchiveEncoding::Utf8),
            "shiftjis" | "sjis" | "cp932" | "windows31j" => Ok(ArchiveEncoding::ShiftJis),
            "gbk" | "gb18030" | "gb2312" | "cp936" => Ok(ArchiveEncoding::Gbk),
            "big5" | "cp950" => Ok(ArchiveEncoding::Big5),
            "euckr" | "cp949" | "korean" => Ok(ArchiveEncoding::EucKr),
            "windows1251" | "cp1251" | "cyrillic" => Ok(ArchiveEncoding::Windows1251),
            "windows1252" | "cp1252" | "latin1" | "iso88591" => Ok(ArchiveEncoding::Windows1252),
            "windows1250" | "cp1250" | "latin2" | "iso88592" => Ok(ArchiveEncoding::Windows1250),
            "cp437" | "ibm437" | "dos" | "oem" => Ok(ArchiveEncoding::Cp437),
            _ => Err(format!("Unknown or unsupported archive encoding: '{s}'")),
        }
    }
}
