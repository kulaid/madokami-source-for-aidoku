#![no_std]
extern crate alloc;

use alloc::string::String;
use crate::alloc::string::ToString
use alloc::vec::Vec;

/// Cleans a filename string (e.g. by trimming whitespace).
/// Adjust this function as needed for your use case.
pub fn clean_filename(input: &str) -> String {
    input.trim().to_string()
}

/// Decodes a URL-encoded string.
/// Replace this stub with your actual URL-decoding logic.
pub fn url_decode(input: &str) -> String {
    input.to_string()
}

/// Holds extracted chapter and volume numbers.
pub struct ChapterInfo {
    pub chapter: f32,
    pub volume: f32,
}

impl Default for ChapterInfo {
    fn default() -> Self {
        Self {
            chapter: 0.0,
            volume: 0.0,
        }
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

/// Checks whether a given manga title is in the exclusion list.
/// (Both the title and the exclusion entries are compared in lowercase.)
fn is_excluded(manga_title: &str) -> bool {
    let exclusions = get_exclusions();
    exclusions.iter().any(|&ex| ex.eq_ignore_ascii_case(manga_title.trim()))
}

/// Parses chapter and volume information from a given filename,
/// using the provided manga title for context.
///
/// If the manga title is in the exclusion list (e.g. titles with numbers that
/// are part of the name), no chapter or volume will be extracted.
pub fn parse_chapter_info(filename: &str, manga_title: &str) -> ChapterInfo {
    let mut info = ChapterInfo::default();

    // Lowercase and clean the filename and manga title.
    let full = clean_filename(&url_decode(filename).to_lowercase());
    let clean_manga = manga_title.to_lowercase();

    // If the manga is in the exclusion list, skip any chapter/volume parsing.
    if is_excluded(&clean_manga) {
        return info;
    }

    // Remove metadata by truncating at " (" if it exists.
    let truncated = if let Some(pos) = full.find(" (") {
        full[..pos].trim()
    } else {
        full.trim()
    };

    // If the truncated name exactly equals the manga title, there's no chapter info.
    if truncated == clean_manga.trim() {
        return info;
    }

    // --- Volume Extraction ---
    // (1) Look for a volume marker in parenthesized form (e.g. "(v15)").
    if let Some(start) = truncated.find("(v") {
        let vol_start = start + 2;
        let vol_str: String = truncated[vol_start..]
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        if !vol_str.is_empty() {
            if let Ok(vol) = vol_str.parse::<f32>() {
                info.volume = vol;
            }
        }
    }
    // (2) Otherwise, check for an unparenthesized marker like " v<digits>".
    else if let Some(pos) = truncated.find(" v") {
        let after = &truncated[pos + 2..];
        let vol_str: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !vol_str.is_empty() {
            if let Ok(vol) = vol_str.parse::<f32>() {
                info.volume = vol;
            }
        }
    }

    // --- Determine the Chapter Section ---
    // If a " - " delimiter is present, assume chapter info is in the substring after it.
    let chapter_section = if let Some(pos) = truncated.rfind(" - ") {
        truncated[pos + 3..].trim()
    } else {
        truncated
    };

    // --- Remove any Volume Marker from the Chapter Section if Present ---
    let chapter_section_clean = if info.volume != 0.0 {
        if let Some(v_pos) = chapter_section.rfind(" v") {
            // Extract the digits after " v" to check if they match the volume.
            let candidate: String = chapter_section[v_pos + 2..]
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect();
            if !candidate.is_empty() {
                if let Ok(num) = candidate.parse::<f32>() {
                    if (num - info.volume).abs() < 0.001 {
                        // If it matches, remove the volume marker.
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

    // (B) Fallback: Extract the trailing group of digits.
    let trailing: String = chapter_section_clean
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    if !trailing.is_empty() {
        if let Ok(num) = trailing.parse::<f32>() {
            // When there’s no " - " delimiter and a volume marker was detected,
            // if the trailing number equals the volume, assume there’s no separate chapter.
            if !truncated.contains(" - ") && info.volume != 0.0 && (num - info.volume).abs() < 0.001 {
                return info;
            } else {
                info.chapter = num;
                return info;
            }
        }
    }

    // (C) Additional Fallback:
    // If there's no delimiter and the truncated name starts with the manga title,
    // remove that prefix and take the leading digits.
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