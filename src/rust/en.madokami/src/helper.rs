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
    let clean_name = clean_filename(&url_decode(filename).to_lowercase());
    let clean_manga = manga_title.to_lowercase();

    if clean_name.trim() == clean_manga.trim() {
        return info;
    }

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

    // Handle volume parsing (unchanged)
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

    // **Fix: Improved chapter detection logic**
    if let Some(pos) = clean_name.find('c') {
        if pos == 0 || !clean_name.as_bytes()[pos - 1].is_ascii_alphabetic() {
            let after = &clean_name[pos + 1..];
            let mut chapter_str = String::new();
            let mut found_digit = false;
            for c in after.chars() {
                if c.is_ascii_digit() {
                    chapter_str.push(c);
                    found_digit = true;
                } else if c == '.' && found_digit {
                    chapter_str.push(c);
                } else if found_digit {
                    break;
                }
            }

            // **Fix: Ensure leading zeros are handled properly**
            if !chapter_str.is_empty() {
                if let Ok(ch) = chapter_str.parse::<f32>() {
                    info.chapter = ch;
                    return info;
                }
            }
        }
    }

    // **Ensure detection of standalone numbers**
    let chars: Vec<char> = clean_name.chars().collect();
    for i in 0..chars.len() {
        if chars[i].is_ascii_digit() && (i == 0 || !chars[i - 1].is_alphanumeric()) {
            let mut number_str = String::new();
            let mut j = i;
            while j < chars.len() && (chars[j].is_ascii_digit() || chars[j] == '.') {
                number_str.push(chars[j]);
                j += 1;
            }
            if !number_str.is_empty() {
                if let Ok(num) = number_str.parse::<f32>() {
                    if num < 1900.0 { // Avoid parsing as a year or large identifier
                        info.chapter = num;
                        return info;
                    }
                }
            }
        }
    }

    // **Fix: Extract trailing number as a last resort**
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