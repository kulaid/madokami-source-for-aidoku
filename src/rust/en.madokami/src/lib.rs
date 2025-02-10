#![no_std]
extern crate alloc;

use aidoku::{
    error::Result,
    prelude::*,
    std::{
        defaults::defaults_get,
        net::{HttpMethod, Request},
        String, Vec,
    },
    Chapter, DeepLink, Filter, FilterType, Manga, MangaPageResult, MangaStatus, MangaViewer, Page,
};
use base64::{engine::general_purpose, Engine};

const BASE_URL: &str = "https://manga.madokami.al";

/// Extracts a manga title from the given path by trimming any leading/trailing slashes,
/// splitting on '/', URL-decoding each segment, and returning the first segment (from the end)
/// that does not start with '!' and does not contain unwanted markers (like "VIZBIG").
///
/// For example, for:
///   "/Manga/V/VI/VINL/Vinland%20Saga/%21Vinland%20Saga%20%5Bdanke-Empire%5D%7BHD%7D"
/// it returns "Vinland Saga".
fn extract_manga_title(path: &str) -> String {
    // Remove any leading/trailing slashes.
    let trimmed = path.trim_matches('/');
    let parts: Vec<&str> = trimmed.split('/').collect();
    for part in parts.iter().rev() {
        if !part.is_empty() {
            let decoded = url_decode(part);
            // Skip if the decoded segment starts with '!' or contains unwanted markers.
            if !decoded.starts_with('!') && !decoded.contains("VIZBIG") {
                return decoded;
            }
        }
    }
    String::new()
}

/// Helper struct to store parsed chapter info.
#[derive(Default)]
struct ChapterInfo {
    /// If this file is a chapter, this will be the chapter number.
    chapter: f32,
    /// If this file is a volume, this will be its volume number.
    volume: f32,
    /// If the filename indicates a range (for example “c001-007”), this holds the start and end.
    chapter_range: Option<(f32, f32)>,
}

/// Adds HTTP Basic authentication headers to a request if credentials are available.
fn add_auth_to_request(request: Request) -> Result<Request> {
    let username = defaults_get("username")?.as_string()?.read();
    let password = defaults_get("password")?.as_string()?.read();
    if !username.is_empty() && !password.is_empty() {
        let auth = format!(
            "Basic {}",
            general_purpose::STANDARD.encode(format!("{}:{}", username, password))
        );
        Ok(request.header("Authorization", &auth))
    } else {
        Ok(request)
    }
}

/// URL-decodes a percent-encoded string.
fn url_decode(input: &str) -> String {
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
fn url_encode(input: &str) -> String {
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
                encoded.push_str(&alloc::format!("%{:02X}", byte));
            }
        }
    }
    encoded
}

/// Revised chapter/volume parser.
///
/// Now accepts the manga title as a second parameter. First, if the cleaned filename starts with
/// the manga title then any trailing digits are taken as the chapter number. Otherwise the
/// parser checks for a volume marker, an explicit chapter marker (e.g. " - c"), an explicit "c"
/// marker, and finally scans for a group of digits preceded by a non-alphanumeric character.
///
/// If a found number is less than 1000 it is assumed to be a chapter number.
fn parse_chapter_info(filename: &str, manga_title: &str) -> ChapterInfo {
    let mut info = ChapterInfo::default();
    let clean_name = url_decode(filename).to_lowercase();
    let clean_manga = manga_title.to_lowercase();

    // (A) If the filename equals the manga title exactly, assume no chapter.
    if clean_name.trim() == clean_manga.trim() {
        return info;
    }

    // (B) If the filename begins with the manga title, try to extract trailing digits.
    if clean_name.starts_with(&clean_manga) {
        let remaining = clean_name[clean_manga.len()..].trim();
        // Extract the first contiguous group of digits (and possibly a dot).
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
            let start_str: String = chapter_part
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect();
            let end_sub = &chapter_part[dash_pos + 1..];
            let end_str: String = end_sub
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect();
            if !start_str.is_empty() && !end_str.is_empty() {
                if let (Ok(start), Ok(end)) = (start_str.parse::<f32>(), end_str.parse::<f32>()) {
                    info.chapter = start;
                    info.chapter_range = Some((start, end));
                    return info;
                }
            }
        } else {
            let chapter_str: String = chapter_part
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect();
            if !chapter_str.is_empty() {
                if let Ok(ch) = chapter_str.parse::<f32>() {
                    info.chapter = ch;
                    return info;
                }
            }
        }
    }

    // (3) Alternatively, if an explicit "c" marker is present (and not part of a word).
    if let Some(pos) = clean_name.find("c") {
        if pos == 0 || !clean_name.as_bytes()[pos - 1].is_ascii_alphabetic() {
            let after = &clean_name[pos + 1..];
            let chapter_str: String = after
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect();
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

/// get_manga_list no longer fetches covers (due to Aidoku limitations), so only minimal data is provided.
#[get_manga_list]
fn get_manga_list(filters: Vec<Filter>, _page: i32) -> Result<MangaPageResult> {
    let mut query = String::new();
    for filter in filters {
        if let FilterType::Title = filter.kind {
            if let Ok(title_ref) = filter.value.as_string() {
                query = url_encode(&title_ref.read());
                break;
            }
        }
    }

    let url = if query.is_empty() {
        format!("{}/recent", BASE_URL)
    } else {
        format!("{}/search?q={}", BASE_URL, query)
    };

    let html = add_auth_to_request(Request::new(url, HttpMethod::Get))?.html()?;
    
    let mut mangas = Vec::new();
    let selector = if query.is_empty() {
        "table.mobile-files-table tbody tr td:nth-child(1) a:nth-child(1)"
    } else {
        "div.container table tbody tr td:nth-child(1) a:nth-child(1)"
    };

    for element in html.select(selector).array() {
        if let Ok(node) = element.as_node() {
            let path = node.attr("href").read();
            if path.ends_with('/') {
                continue;
            }
            // Use extract_manga_title to derive the title from the full path.
            let title = extract_manga_title(&path);
            mangas.push(Manga {
                id: path.clone(),
                title,
                cover: String::new(), // Empty cover URL for performance
                url: format!("{}{}", BASE_URL, path),
                status: MangaStatus::Unknown,
                viewer: MangaViewer::Rtl,
                ..Default::default()
            });
        }
    }

    Ok(MangaPageResult {
        manga: mangas,
        has_more: false,
    })
}

/// In get_chapter_list we now pass the manga title to the chapter parser so that filenames like
/// "Hunter x Hunter 400 (2022) (Digital) (LuCaZ).cbz" are correctly classified as chapters.
#[get_chapter_list]
fn get_chapter_list(id: String) -> Result<Vec<Chapter>> {
    let html = add_auth_to_request(Request::new(format!("{}{}", BASE_URL, id), HttpMethod::Get))?.html()?;
    let mut chapters = Vec::new();
    // Extract the manga title from the id.
    let manga_title = extract_manga_title(&id);
    
    for row in html.select("table#index-table > tbody > tr").array() {
        if let Ok(node) = row.as_node() {
            let title_node = node.select("td:nth-child(1) a");
            let title = title_node.text().read();
            
            // Skip entries that are directories or start with '!'
            if title.ends_with('/') || title.starts_with('!') {
                continue;
            }

            let read_link = node.select("td:nth-child(6) a").first();
            let base_url = read_link.attr("href").read();
            let url = if let Some(reader_part) = base_url.split("/reader").last() {
                format!("/reader{}", reader_part)
            } else {
                continue;
            };

            // Parse the chapter/volume information from the filename,
            // passing the manga title so that numbers appended after the title are captured.
            let info = parse_chapter_info(&title, &manga_title);
            let date_updated = node
                .select("td:nth-child(3)")
                .text()
                .as_date("yyyy-MM-dd HH:mm", None, None);
            
            if let Some((start, end)) = info.chapter_range {
                // If a range is given, create an entry for each chapter number.
                for ch in (start as i32)..=(end as i32) {
                    chapters.push(Chapter {
                        id: url.clone(),
                        title: format!("Chapter {}", ch),
                        chapter: ch as f32,
                        volume: if info.volume > 0.0 { info.volume } else { -1.0 },
                        date_updated,
                        url: format!("{}{}", BASE_URL, url),
                        ..Default::default()
                    });
                }
            } else {
                chapters.push(Chapter {
                    id: url.clone(),
                    title: url_decode(&title),
                    chapter: if info.chapter > 0.0 { info.chapter } else { -1.0 },
                    volume: if info.volume > 0.0 { info.volume } else { -1.0 },
                    date_updated,
                    url: format!("{}{}", BASE_URL, url),
                    ..Default::default()
                });
            }
        }
    }

    chapters.reverse();
    Ok(chapters)
}

#[get_manga_details]
fn get_manga_details(id: String) -> Result<Manga> {
    let html = add_auth_to_request(Request::new(format!("{}{}", BASE_URL, id), HttpMethod::Get))?.html()?;
    
    let mut authors = Vec::new();
    let mut genres = Vec::new();
    let mut status = MangaStatus::Unknown;
    let cover_url = html.select("div.manga-info img[itemprop=\"image\"]").attr("src").read();

    for author_node in html.select("a[itemprop=\"author\"]").array() {
        if let Ok(node) = author_node.as_node() {
            authors.push(node.text().read());
        }
    }
    for genre_node in html.select("div.genres a.tag").array() {
        if let Ok(node) = genre_node.as_node() {
            genres.push(node.text().read());
        }
    }
    if html.select("span.scanstatus").text().read() == "Yes" {
        status = MangaStatus::Completed;
    }
    
    Ok(Manga {
        id: id.clone(),
        // Use extract_manga_title to derive a clean title from the id.
        title: extract_manga_title(&id),
        author: authors.join(", "),
        cover: cover_url,
        categories: genres,
        status,
        url: format!("{}{}", BASE_URL, id),
        viewer: MangaViewer::Rtl,
        ..Default::default()
    })
}

#[get_page_list]
fn get_page_list(_manga_id: String, chapter_id: String) -> Result<Vec<Page>> {
    let html = add_auth_to_request(Request::new(format!("{}{}", BASE_URL, chapter_id), HttpMethod::Get))?.html()?;
    let reader = html.select("div#reader");
    let path = reader.attr("data-path").read();
    let files = reader.attr("data-files").read();
    
    let mut pages = Vec::new();
    if let Ok(file_list) = aidoku::std::json::parse(files.as_bytes()) {
        if let Ok(array) = file_list.as_array() {
            for (index, file) in array.enumerate() {
                if let Ok(filename) = file.as_string() {
                    pages.push(Page {
                        index: index as i32,
                        url: format!(
                            "{}/reader/image?path={}&file={}",
                            BASE_URL,
                            url_encode(&path),
                            url_encode(&filename.read())
                        ),
                        ..Default::default()
                    });
                }
            }
        }
    }
    
    Ok(pages)
}

#[modify_image_request]
fn modify_image_request(request: Request) {
    if let Ok(request_with_auth) = add_auth_to_request(request) {
        request_with_auth
            .header("Referer", BASE_URL)
            .header("Accept", "image/*");
    }
}

#[handle_url]
fn handle_url(url: String) -> Result<DeepLink> {
    let url = url.replace(BASE_URL, "");
    if url.starts_with("/reader") {
        Ok(DeepLink {
            manga: Some(Manga {
                id: String::from(url.split("/reader").next().unwrap_or_default()),
                ..Default::default()
            }),
            chapter: Some(Chapter {
                id: url,
                ..Default::default()
            }),
        })
    } else {
        Ok(DeepLink {
            manga: Some(Manga {
                id: url,
                ..Default::default()
            }),
            ..Default::default()
        })
    }
}