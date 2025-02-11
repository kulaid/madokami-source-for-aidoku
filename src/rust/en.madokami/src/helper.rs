use aidoku::std::String;
use alloc::{string::ToString, vec::Vec, format};

/// Public struct to store parsed chapter info.
#[derive(Default)]
pub struct ChapterInfo {
    /// If this file is a chapter, this will be the chapter number.
    pub chapter: f32,
    /// If this file is a volume, this will be its volume number.
    pub volume: f32,
    /// If the filename indicates a range (for example "c001-007"), this holds the start and end.
    pub chapter_range: Option<(f32, f32)>,
}

/// Decodes HTML entities in the input string
pub fn decode_html_entities(input: &str) -> String {
    input
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#039;", "'")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// Removes JavaScript and cleans HTML from description
pub fn clean_description(input: &str) -> String {
    // First decode any HTML entities
    let decoded = decode_html_entities(input);
    
    // Remove any JavaScript section
    if let Some(end_idx) = decoded.find("//-->") {
        if let Some(start_idx) = decoded[..end_idx].rfind("<!--") {
            // Get everything after the JavaScript
            let after_script = decoded[end_idx + 5..].trim();
            if !after_script.is_empty() {
                return after_script.to_string();
            }
        }
    }
    
    // If no script found or nothing after script, return decoded input
    decoded
}

/// URL-decodes a percent-encoded string.
pub fn url_decode(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut i = 0;
    let bytes = input.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h1), Some(h2)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                result.push((h1 << 4 | h2) as char);
                i += 3;
                continue;
            }
        }
        result.push(bytes[i] as char);
        i += 1;
    }
    result
}

/// Returns the numerical value of a hexadecimal digit.
fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// URL-encodes a string.
pub fn url_encode(input: &str) -> String {
    let mut encoded = String::new();
    for byte in input.bytes() {
        match byte {
            b'a'..=b'z'
            | b'A'..=b'Z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'!'
            | b'~'
            | b'*'
            | b'\'' => {
                encoded.push(byte as char);
            }
            _ => {
                encoded.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    encoded
}

/// Removes common manga archive extensions from the filename.
pub fn clean_filename(filename: &str) -> String {
    // Common extensions to remove.
    const EXTENSIONS: &[&str] = &[
        ".cbz", ".zip", ".cbr", ".rar", ".7z", ".pdf", ".epub", 
        ".png", ".jpg", ".jpeg", ".gif", ".xml", ".txt"
    ];

    let mut cleaned = filename.to_string();
    let cleaned_lower = cleaned.to_lowercase();
    
    // Remove any of the specified extensions.
    for ext in EXTENSIONS {
        if cleaned_lower.ends_with(ext) {
            cleaned.truncate(cleaned.len() - ext.len());
            break;
        }
    }

    cleaned
}

/// Extracts a manga title from the given path by trimming leading/trailing slashes,
/// splitting on '/', URL-decoding each segment, cleaning it, and returning the first segment
/// (from the end) that does not start with '!' and does not contain unwanted markers.
pub fn extract_manga_title(path: &str) -> String {
    let trimmed = path.trim_matches('/');
    let parts: Vec<&str> = trimmed.split('/').collect();
    for part in parts.iter().rev() {
        if !part.is_empty() {
            let decoded = url_decode(part);
            if !decoded.starts_with('!') && !decoded.contains("VIZBIG") {
                return clean_filename(&decoded);
            }
        }
    }
    String::new()
}

/// Returns the parent path of the given path by collecting its segments until a segment
/// with unwanted markers (like "VIZBIG" or one starting with '!') is encountered.
pub fn get_parent_path(path: &str) -> Option<String> {
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let mut parent_parts = Vec::new();
    for part in parts {
        let decoded = url_decode(part);
        if decoded.contains("VIZBIG") || decoded.starts_with('!') {
            break;
        }
        parent_parts.push(part);
    }
    if !parent_parts.is_empty() {
        Some(format!("/{}", parent_parts.join("/")))
    } else {
        None
    }
}

/// Parses chapter/volume information from a filename given the manga title.
pub fn parse_chapter_info(filename: &str, manga_title: &str) -> ChapterInfo {
    let mut info = ChapterInfo::default();
    let clean_name = url_decode(filename).to_lowercase();
    let clean_name = clean_filename(&clean_name);
    let clean_manga = manga_title.to_lowercase();

    // (A) If the filename equals the manga title exactly, assume no chapter.
    if clean_name.trim() == clean_manga.trim() {
        return info;
    }

    // (B) If the filename begins with the manga title, try to extract trailing digits.
    if clean_name.starts_with(&clean_manga) {
        let remaining = clean_name[clean_manga.len()..].trim();
        let digits: String = remaining.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
        if !digits.is_empty() {
            if let Ok(num) = digits.parse::<f32>() {
                info.chapter = num;
                return info;
            }
        }
    }

    // (1) Check for a volume marker (e.g. " v01").
    if let Some(pos) = clean_name.find(" v") {
        let after = &clean_name[pos + 2..];
        let vol_str: String = after.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
        if !vol_str.is_empty() {
            if let Ok(vol) = vol_str.parse::<f32>() {
                info.volume = vol;
                return info;
            }
        }
    }

    // (2) Look for an explicit chapter marker (" - c").
    if let Some(pos) = clean_name.find(" - c") {
        let chapter_part = &clean_name[pos + 4..]; // Skip " - c"
        if let Some(dash_pos) = chapter_part.find('-') {
            let start_str: String = chapter_part.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
            let end_sub = &chapter_part[dash_pos + 1..];
            let end_str: String = end_sub.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
            if !start_str.is_empty() && !end_str.is_empty() {
                if let (Ok(start), Ok(end)) = (start_str.parse::<f32>(), end_str.parse::<f32>()) {
                    info.chapter = start;
                    info.chapter_range = Some((start, end));
                    return info;
                }
            }
        } else {
            let chapter_str: String = chapter_part.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
            if !chapter_str.is_empty() {
                if let Ok(ch) = chapter_str.parse::<f32>() {
                    info.chapter = ch;
                    return info;
                }
            }
        }
    }

    // (3) Alternatively, if an explicit "c" marker is present (and not part of a word).
    if let Some(pos) = clean_name.find('c') {
        if pos == 0 || !clean_name.as_bytes()[pos - 1].is_ascii_alphabetic() {
            let after = &clean_name[pos + 1..];
            let chapter_str: String = after.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
            if !chapter_str.is_empty() {
                if let Ok(ch) = chapter_str.parse::<f32>() {
                    info.chapter = ch;
                    return info;
                }
            }
        }
    }

    // (4) Fallback: scan for a group of digits that appears after a non-alphanumeric delimiter.
    let chars: Vec<char> = clean_name.chars().collect();
    for i in 0..chars.len() {
        if chars[i].is_ascii_digit() {
            if i == 0 || !chars[i - 1].is_alphanumeric() {
                let mut number_str = String::new();
                let mut j = i;
                while j < chars.len() && (chars[j].is_ascii_digit() || chars[j] == '.') {
                    number_str.push(chars[j]);
                    j += 1;
                }
                if !number_str.is_empty() {
                    if let Ok(num) = number_str.parse::<f32>() {
                        if num < 1000.0 {
                            info.chapter = num;
                            return info;
                        }
                    }
                }
            }
        }
    }

    // (5) Final fallback: extract trailing digits.
    let trailing: String = clean_name
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    if !trailing.is_empty() {
        if let Ok(ch) = trailing.parse::<f32>() {
            info.chapter = ch;
            return info;
        }
    }

    info
}
