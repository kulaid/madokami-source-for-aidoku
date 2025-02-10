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

mod helper;
use helper::*;

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
            // Use extract_manga_title to derive the title from the full path
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

#[get_chapter_list]
fn get_chapter_list(id: String) -> Result<Vec<Chapter>> {
    let html = add_auth_to_request(Request::new(format!("{}{}", BASE_URL, id), HttpMethod::Get))?.html()?;
    let mut chapters = Vec::new();
    // Extract the manga title from the id
    let manga_title = extract_manga_title(&id);
    
    for row in html.select("table#index-table > tbody > tr").array() {
        if let Ok(node) = row.as_node() {
            let title_node = node.select("td:nth-child(1) a");
            let title = clean_filename(&title_node.text().read());
            
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

            // Parse the chapter/volume information from the filename
            let info = parse_chapter_info(&title, &manga_title);
            let date_updated = node
                .select("td:nth-child(3)")
                .text()
                .as_date("yyyy-MM-dd HH:mm", None, None);
            
            if let Some((start, end)) = info.chapter_range {
                // If a range is given, create an entry for each chapter number
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
                    title: clean_filename(&url_decode(&title)),
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