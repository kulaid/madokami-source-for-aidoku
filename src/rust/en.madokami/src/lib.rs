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
use alloc::format;

mod helper;
use helper::*;

const BASE_URL: &str = "https://manga.madokami.al";

/// Adds HTTP Basic authentication to the given request if credentials are provided.
fn add_auth_to_request(mut request: Request) -> Request {
    let username = defaults_get("username")
        .and_then(|v| v.as_string())
        .map(|s| s.read())
        .unwrap_or_default();
    let password = defaults_get("password")
        .and_then(|v| v.as_string())
        .map(|s| s.read())
        .unwrap_or_default();

    if !username.is_empty() && !password.is_empty() {
        let auth = format!(
            "Basic {}",
            general_purpose::STANDARD.encode(format!("{}:{}", username, password))
        );
        request = request.header("Authorization", &auth);
    }
    request
}

#[get_manga_list]
fn get_manga_list(filters: Vec<Filter>, _page: i32) -> Result<MangaPageResult> {
    // Build URL based on whether a title filter is provided.
    let url = if let Some(query) = filters
        .into_iter()
        .find(|f| matches!(f.kind, FilterType::Title))
        .and_then(|f| f.value.as_string().ok())
        .map(|s| url_encode(&s.read()))
    {
        format!("{}/search?q={}", BASE_URL, query)
    } else {
        format!("{}/recent", BASE_URL)
    };

    let html = add_auth_to_request(Request::new(url.clone(), HttpMethod::Get)).html()?;

    // Use a different CSS selector depending on the URL.
    let selector = if url.ends_with("/recent") {
        "table.mobile-files-table tbody tr td:nth-child(1) a:nth-child(1)"
    } else {
        "div.container table tbody tr td:nth-child(1) a:nth-child(1)"
    };

    // Build a list of manga entries.
    let mangas = html
        .select(selector)
        .array()
        .filter_map(|element| element.as_node().ok())
        .filter_map(|node| {
            let path = node.attr("href").read();
            if path.ends_with('/') {
                None
            } else {
                Some(Manga {
                    id: path.clone(),
                    title: extract_manga_title(&path),
                    cover: String::new(),
                    url: format!("{}{}", BASE_URL, path),
                    status: MangaStatus::Unknown,
                    viewer: MangaViewer::Rtl,
                    ..Default::default()
                })
            }
        })
        .collect::<Vec<Manga>>();

    Ok(MangaPageResult {
        manga: mangas,
        has_more: false,
    })
}

#[get_chapter_list]
fn get_chapter_list(id: String) -> Result<Vec<Chapter>> {
    let html = add_auth_to_request(
        Request::new(format!("{}{}", BASE_URL, id), HttpMethod::Get)
    )
    .html()?;
    let manga_title = extract_manga_title(&id);
    let mut chapters = Vec::new();

    for row in html.select("table#index-table > tbody > tr").array() {
        if let Ok(node) = row.as_node() {
            let title = node.select("td:nth-child(1) a").text().read();
            if title.ends_with('/') || title.starts_with('!') {
                continue;
            }
            let base_url = node.select("td:nth-child(6) a").first().attr("href").read();
            let url = match base_url.split("/reader").last() {
                Some(reader_part) => format!("/reader{}", reader_part),
                None => continue,
            };
            let date_updated = node
                .select("td:nth-child(3)")
                .text()
                .as_date("yyyy-MM-dd HH:mm", None, None);
            let info = parse_chapter_info(&title, &manga_title);
            let chapter_number = if info.chapter > 0.0 { info.chapter } else { -1.0 };

            chapters.push(Chapter {
                id: url.clone(),
                title: clean_filename(&url_decode(&title)),
                chapter: chapter_number,
                volume: if info.volume > 0.0 { info.volume } else { -1.0 },
                date_updated,
                url: format!("{}{}", BASE_URL, url),
                ..Default::default()
            });
        }
    }
    chapters.reverse();
    Ok(chapters)
}

#[get_manga_details]
fn get_manga_details(id: String) -> Result<Manga> {
    let mut authors = Vec::new();
    let mut genres = Vec::new();
    let mut status = MangaStatus::Unknown;
    let mut cover_url = String::new();
    let mut parent_description = String::new();

    // Use the last segment of the path (after trimming) as the directory name.
    let dir_name = id.trim_matches('/').rsplit('/').next().map(url_decode).unwrap_or_default();

    if let Some(parent_path) = get_parent_path(&id) {
        if let Ok(parent_html) = add_auth_to_request(
            Request::new(format!("{}{}", BASE_URL, parent_path), HttpMethod::Get)
        ).html() {
            cover_url = parent_html
                .select("div.manga-info img[itemprop=\"image\"]")
                .attr("src")
                .read();

            authors = parent_html
                .select("a[itemprop=\"author\"]")
                .array()
                .filter_map(|n| n.as_node().ok().map(|node| node.text().read()))
                .collect();

            genres = parent_html
                .select("div.genres a.tag")
                .array()
                .filter_map(|n| n.as_node().ok().map(|node| node.text().read()))
                .collect();

            parent_description = {
                let og_desc = parent_html
                    .select("meta[property=\"og:description\"]")
                    .attr("content")
                    .read();
                let desc = if !og_desc.is_empty() {
                    og_desc
                } else {
                    parent_html
                        .select("meta[name=\"description\"]")
                        .attr("content")
                        .read()
                };
                clean_description(&desc)
            };

            if parent_html.select("span.scanstatus").text().read() == "Yes" {
                status = MangaStatus::Completed;
            }
        }
    }

    let description = if !dir_name.is_empty() && !parent_description.is_empty() {
        format!("{}\n\n{}", dir_name, parent_description)
    } else if !dir_name.is_empty() {
        dir_name
    } else {
        parent_description
    };

    Ok(Manga {
        id: id.clone(),
        title: extract_manga_title(&id),
        author: authors.join(", "),
        cover: cover_url,
        categories: genres,
        status,
        description,
        url: format!("{}{}", BASE_URL, id),
        viewer: MangaViewer::Rtl,
        ..Default::default()
    })
}

#[get_page_list]
fn get_page_list(_manga_id: String, chapter_id: String) -> Result<Vec<Page>> {
    // Remove any extra query parameter from the chapter_id.
    let chapter_id = chapter_id.split("?ch=").next().unwrap_or(&chapter_id);
    let html = add_auth_to_request(
        Request::new(format!("{}{}", BASE_URL, chapter_id), HttpMethod::Get)
    )
    .html()?;

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
fn modify_image_request(request: Request) -> Request {
    add_auth_to_request(request)
        .header("Referer", BASE_URL)
        .header("Accept", "image/*")
}