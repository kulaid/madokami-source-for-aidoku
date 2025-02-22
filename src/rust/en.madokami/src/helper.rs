use aidoku::std::String;
use alloc::{vec::Vec, format};
use alloc::string::ToString;

#[derive(Default)]
pub struct ChapterInfo {
    pub chapter: f32,
    pub volume: f32,
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

/// Loads an exclusion list from an external file at compile time.
/// This file (exclusions.txt) should be in the same directory as helper.rs.
fn get_exclusions() -> Vec<&'static str> {
    include_str!("exclusions.txt")
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect()
}

/// Removes the manga title from the filename if it's in the exclusion list
fn remove_excluded_title(filename: &str, manga_title: &str) -> String {
    let exclusions = get_exclusions();
    let lower_filename = filename.to_lowercase();
    let lower_title = manga_title.trim().to_lowercase();
    
    if !exclusions.iter().any(|&ex| ex.eq_ignore_ascii_case(&lower_title)) {
        return filename.to_string();
    }
    
    // If the filename starts with the manga title, remove it
    if lower_filename.starts_with(&lower_title) {
        return filename[manga_title.len()..].trim().to_string();
    }
    
    filename.to_string()
}

/// Parses chapter and volume information from a given filename,
/// using the provided manga title for context.
pub fn parse_chapter_info(filename: &str, manga_title: &str) -> ChapterInfo {
    let mut info = ChapterInfo::default();

    // Lowercase and clean the filename and manga title
    let full = clean_filename(&url_decode(filename).to_lowercase());
    let clean_manga = manga_title.to_lowercase();
    
    // Remove the title if it's in the exclusion list
    let processed = remove_excluded_title(&full, manga_title);

    // Remove metadata by truncating at " (" if it exists
    let truncated = if let Some(pos) = processed.find(" (") {
        processed[..pos].trim()
    } else {
        processed.trim()
    };

    // If the truncated name exactly equals the manga title, there's no chapter info
    if truncated == clean_manga.trim() {
        return info;
    }

    // --- Volume Extraction ---
    // Iterate through each 'v' occurrence and only accept one that is followed by digits.
    let lower_truncated = truncated.to_lowercase();
    let mut search_index = 0;
    while let Some(pos) = lower_truncated[search_index..].find('v') {
        let pos = search_index + pos;
        let after_v = &truncated[pos + 1..];
        let after_v_trim = after_v.trim_start();
        if let Some(first_char) = after_v_trim.chars().next() {
            if first_char.is_ascii_digit() {
                // Valid volume marker found!
                let vol_str: String = after_v_trim
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect();
                if !vol_str.is_empty() {
                    if let Ok(vol) = vol_str.parse::<f32>() {
                        info.volume = vol;
                        break;
                    }
                }
            }
        }
        search_index = pos + 1;
    }

    // --- Determine the Chapter Section ---
    let chapter_section = if let Some(pos) = truncated.rfind(" - ") {
        truncated[pos + 3..].trim()
    } else {
        truncated
    };

    // --- Remove any Volume Marker from the Chapter Section if Present ---
    let chapter_section_clean = if info.volume != 0.0 {
        if let Some(v_pos) = chapter_section.rfind(" v") {
            let candidate: String = chapter_section[v_pos + 2..]
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect();
            if !candidate.is_empty() {
                if let Ok(num) = candidate.parse::<f32>() {
                    if (num - info.volume).abs() < 0.001 {
                        chapter_section[..v_pos].trim().to_string()
                    } else {
                        chapter_section.to_string()
                    }
                } else {
                    chapter_section.to_string()
                }
            } else {
                chapter_section.to_string()
            }
        } else {
            chapter_section.to_string()
        }
    } else {
        chapter_section.to_string()
    };

    // --- Chapter Extraction ---
    // (A) If the cleaned chapter section explicitly starts with 'c',
    // extract the digits immediately following.
    if chapter_section_clean.starts_with('c') {
        let after_c = chapter_section_clean[1..].trim_start();
        let digits: String = after_c.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !digits.is_empty() {
            if let Ok(num) = digits.parse::<f32>() {
                info.chapter = num;
                return info;
            }
        }
    }

    // (B) Fallback: Extract the trailing number (including decimals)
    let end_idx = chapter_section_clean.len();
    let mut start_idx = end_idx;
    let chars: Vec<char> = chapter_section_clean.chars().collect();
    
    // Find the start of the trailing number (walking backwards)
    while start_idx > 0 && (chars[start_idx - 1].is_ascii_digit() || chars[start_idx - 1] == '.') {
        start_idx -= 1;
    }
    
    if start_idx < end_idx {
        let number_str = &chapter_section_clean[start_idx..end_idx];
        // Only parse if it starts with a digit (avoid parsing just ".")
        if number_str.chars().next().map_or(false, |c| c.is_ascii_digit()) {
            if let Ok(num) = number_str.parse::<f32>() {
                if !truncated.contains(" - ") && info.volume != 0.0 && (num - info.volume).abs() < 0.001 {
                    return info;
                } else {
                    info.chapter = num;
                    return info;
                }
            }
        }
    }

    // (C) Additional Fallback: If the truncated string starts with the manga title
    if !truncated.contains(" - ") && truncated.starts_with(&clean_manga) {
        let remaining = truncated[clean_manga.len()..].trim();
        let digits: String = remaining.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !digits.is_empty() {
            if let Ok(num) = digits.parse::<f32>() {
                info.chapter = num;
                return info;
            }
        }
    }

    info
}
