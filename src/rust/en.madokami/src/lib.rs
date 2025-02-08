#![no_std]
extern crate alloc;

use alloc::string::ToString;
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

/// Extracts a manga title from the given path by iterating backwards through path segments.
fn extract_manga_title(path: &str) -> String {
    let parts: Vec<&str> = path.split('/').collect();
    let mut found_title = String::new();
    for part in parts.iter().rev() {
        if !part.is_empty() {
            let decoded = url_decode(part);
            found_title = decoded.clone();
            if !decoded.starts_with('!') {
                return found_title;
            }
        }
    }
    found_title
}

/// Decodes a URL-encoded string.
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

/// Converts a hexadecimal character into its numerical value.
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

/// Returns the parent path of the given path, if available.
fn get_parent_path(path: &str) -> Option<String> {
    let trimmed = path.trim_end_matches('/');
    if let Some(index) = trimmed.rfind('/') {
        if index == 0 {
            Some(String::from("/"))
        } else {
            Some(trimmed[..index].to_string())
        }
    } else {
        None
    }
}

#[get_manga_list]
fn get_manga_list(filters: Vec<Filter>, _page: i32) -> Result<MangaPageResult> {
    // Build the search query if available.
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
    let selector = if query.is_empty() {
        "table.mobile-files-table tbody tr td:nth-child(1) a:nth-child(1)"
    } else {
        "div.container table tbody tr td:nth-child(1) a:nth-child(1)"
    };

    let mut mangas = Vec::new();
    // Iterate over manga entries using only the list page's HTML.
    for element in html.select(selector).array() {
        if let Ok(node) = element.as_node() {
            let path = node.attr("href").read();
            // Skip directory entries.
            if path.ends_with('/') {
                continue;
            }
            let title = extract_manga_title(&path);
            // Use the <img> tag if available; otherwise, leave the cover empty.
            let cover = node.select("img").attr("src").read();
            mangas.push(Manga {
                id: path.clone(),
                title,
                cover,
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

#[get_manga_details]
fn get_manga_details(id: String) -> Result<Manga> {
    // When a manga is selected, load its details page.
    let mut html = add_auth_to_request(Request::new(format!("{}{}", BASE_URL, id), HttpMethod::Get))?.html()?;
    let mut authors = Vec::new();
    let mut genres = Vec::new();
    let mut status = MangaStatus::Unknown;
    let mut cover_url = html
        .select("div.manga-info img[itemprop=\"image\"]")
        .attr("src")
        .read();
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
    // If key metadata is missing, try the parent directory.
    if authors.is_empty() || genres.is_empty() || cover_url.is_empty() {
        let parts: Vec<&str> = id.split('/').filter(|s| !s.is_empty()).collect();
        if parts.len() > 1 {
            let mut parent_parts = Vec::new();
            let mut found_parent = false;
            for part in parts.iter() {
                let decoded = url_decode(part);
                if !decoded.starts_with('!') {
                    parent_parts.push(*part);
                    found_parent = true;
                } else if !found_parent {
                    parent_parts.push(*part);
                }
            }
            if !parent_parts.is_empty() {
                let parent_path = format!("/{}", parent_parts.join("/"));
                if let Ok(parent_html) = add_auth_to_request(
                    Request::new(format!("{}{}", BASE_URL, parent_path), HttpMethod::Get)
                )?.html() {
                    html = parent_html;
                    if cover_url.is_empty() {
                        cover_url = html
                            .select("div.manga-info img[itemprop=\"image\"]")
                            .attr("src")
                            .read();
                    }
                    if authors.is_empty() {
                        for author_node in html.select("a[itemprop=\"author\"]").array() {
                            if let Ok(node) = author_node.as_node() {
                                authors.push(node.text().read());
                            }
                        }
                    }
                    if genres.is_empty() {
                        for genre_node in html.select("div.genres a.tag").array() {
                            if let Ok(node) = genre_node.as_node() {
                                genres.push(node.text().read());
                            }
                        }
                    }
                    if status == MangaStatus::Unknown && html.select("span.scanstatus").text().read() == "Yes" {
                        status = MangaStatus::Completed;
                    }
                }
            }
        }
    }
    Ok(Manga {
        id: id.clone(),
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

#[get_chapter_list]
fn get_chapter_list(id: String) -> Result<Vec<Chapter>> {
    let html = add_auth_to_request(Request::new(format!("{}{}", BASE_URL, id), HttpMethod::Get))?.html()?;
    let mut chapters = Vec::new();
    for row in html.select("table#index-table > tbody > tr").array() {
        if let Ok(node) = row.as_node() {
            let title_node = node.select("td:nth-child(1) a");
            let title = title_node.text().read();
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
            chapters.push(Chapter {
                id: url.clone(),
                title: url_decode(&title),
                chapter: -1.0,
                date_updated: node
                    .select("td:nth-child(3)")
                    .text()
                    .as_date("yyyy-MM-dd HH:mm", None, None),
                url: format!("{}{}", BASE_URL, url),
                ..Default::default()
            });
        }
    }
    chapters.reverse();
    Ok(chapters)
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
