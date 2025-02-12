use aidoku::std::String;
use alloc::{vec::Vec, format};
// Bring the ToString trait into scope so that .to_string() works.
use alloc::string::ToString;

#[derive(Default)]
pub struct ChapterInfo {
    pub chapter: f32,
    pub volume: f32,
    // This field remains available for future use.
    pub chapter_range: Option<(f32, f32)>,
}

pub fn decode_html_entities(input: &str) -> String {
    input
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#039;", "'")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

pub fn clean_description(input: &str) -> String {
    let decoded = decode_html_entities(input);
    if let Some(end_idx) = decoded.find("//-->") {
        if let Some(_start_idx) = decoded[..end_idx].rfind("<!--") {
            let after_script = decoded[end_idx + 5..].trim();
            if !after_script.is_empty() {
                return after_script.to_string();
            }
        }
    }
    decoded
}

pub fn url_decode(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
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

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

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
            | b'\'' => encoded.push(byte as char),
            _ => encoded.push_str(&format!("%{:02X}", byte)),
        }
    }
    encoded
}

pub fn clean_filename(filename: &str) -> String {
    const EXTENSIONS: &[&str] = &[
        ".cbz", ".zip", ".cbr", ".rar", ".7z", ".pdf", ".epub",
        ".png", ".jpg", ".jpeg", ".gif", ".xml", ".txt",
    ];
    let mut cleaned = filename.to_string();
    let cleaned_lower = cleaned.to_lowercase();
    for ext in EXTENSIONS {
        if cleaned_lower.ends_with(ext) {
            cleaned.truncate(cleaned.len() - ext.len());
            break;
        }
    }
    cleaned
}

pub fn extract_manga_title(path: &str) -> String {
    path.trim_matches('/')
        .split('/')
        .rev()
        .filter(|part| !part.is_empty())
        .map(url_decode)
        .find(|decoded| !decoded.starts_with('!') && !decoded.contains("VIZBIG"))
        .map(|decoded| clean_filename(&decoded))
        .unwrap_or_default()
}

pub fn get_parent_path(path: &str) -> Option<String> {
    let parent_parts: Vec<&str> = path
        .split('/')
        .filter(|s| !s.is_empty())
        .take_while(|part| {
            let decoded = url_decode(part);
            !decoded.contains("VIZBIG") && !decoded.starts_with('!')
        })
        .collect();
    if parent_parts.is_empty() {
        None
    } else {
        Some(format!("/{}", parent_parts.join("/")))
    }
}

pub fn parse_chapter_info(filename: &str, manga_title: &str) -> ChapterInfo {
    let mut info = ChapterInfo::default();

    // Decode, lowercase, and clean the filename and manga title.
    let clean_name = clean_filename(&url_decode(filename).to_lowercase());
    let clean_manga = manga_title.to_lowercase();

    // If the filename equals the manga title exactly, there's no chapter info.
    if clean_name.trim() == clean_manga.trim() {
        return info;
    }

    // --- Volume Extraction ---
    // First, try to detect a volume marker in the form "(v<digits>)"
    if let Some(start) = clean_name.find("(v") {
        let vol_start = start + 2;
        let vol_str: String = clean_name[vol_start..]
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        if !vol_str.is_empty() {
            if let Ok(vol) = vol_str.parse::<f32>() {
                info.volume = vol;
            }
        }
    }
    // If not found, also check for an unparenthesized volume marker like " v<digits>"
    else if let Some(pos) = clean_name.find(" v") {
        let after = &clean_name[pos + 2..];
        let vol_str: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !vol_str.is_empty() {
            if let Ok(vol) = vol_str.parse::<f32>() {
                info.volume = vol;
            }
        }
    }

    // --- Determine the Chapter Section ---
    // If the filename contains " - ", assume that the chapter info is in the last segment.
    let chapter_section = if let Some(pos) = clean_name.rfind(" - ") {
        clean_name[pos + 3..].trim()
    } else {
        clean_name.trim()
    };

    // --- Chapter Marker Extraction ---
    // If the chapter section explicitly starts with 'c', extract digits immediately following it.
    if chapter_section.starts_with('c') {
        let after_c = chapter_section[1..].trim_start();
        let chapter_digits: String = after_c.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !chapter_digits.is_empty() {
            if let Ok(num) = chapter_digits.parse::<f32>() {
                info.chapter = num;
                return info;
            }
        }
    }

    // --- Fallback: Use Trailing Digits from the Chapter Section ---
    let trailing: String = chapter_section
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    if !trailing.is_empty() {
        if let Ok(num) = trailing.parse::<f32>() {
            info.chapter = num;
            return info;
        }
    }

    // --- Additional Fallback ---
    // If there is no " - " delimiter and the filename starts with the manga title,
    // remove that portion and then take the leading digits from what remains.
    if !clean_name.contains(" - ") && clean_name.starts_with(&clean_manga) {
        let remaining = clean_name[clean_manga.len()..].trim();
        let digits: String = remaining
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect();
        if !digits.is_empty() {
            if let Ok(num) = digits.parse::<f32>() {
                info.chapter = num;
                return info;
            }
        }
    }

    info
}
