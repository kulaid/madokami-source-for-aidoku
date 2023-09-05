use aidoku::{
	error::Result, prelude::*, std::html::Node, std::String, std::Vec, Chapter, Filter, FilterType,
	Manga, MangaContentRating, MangaStatus, MangaViewer, Page,
};
extern crate alloc;
use alloc::string::ToString;

pub const BASE_URL: &str = "https://w.mangairo.com";
pub const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 13_3_1) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/113.0.0.0 Safari/537.36";

pub fn parse_manga_list(html: Node, result: &mut Vec<Manga>) {
	for page in html.select(".story-item").array() {
		let obj = page.as_node().expect("node array");

		let id = obj.select(".story-name a").attr("href").read();
		let title = obj.select(".story-name a ").text().read();
		let img = obj.select(".story-list-img img").attr("src").read();

		if !id.is_empty() && !title.is_empty() && !img.is_empty() {
			result.push(Manga {
				id,
				cover: img,
				title,
				..Default::default()
			});
		}
	}
}

pub fn parse_manga_details(html: Node, id: String) -> Result<Manga> {
	let title = html
		.select(".breadcrumbs p span a span")
		.last()
		.text()
		.read();
	let cover = html.select(".avatar").attr("src").read();
	let description = html
		.select("div#story_discription p")
		.text()
		.read()
		.trim()
		.to_string();

	let url = format!("https://chap.mangairo.com/{}", &id);

	// TODO:
	let categories: Vec<String> = Vec::new();
	let status = MangaStatus::Ongoing;
	let nsfw = MangaContentRating::Safe;
	let viewer = MangaViewer::Rtl;

	Ok(Manga {
		id,
		cover,
		title,
		description,
		url,
		categories,
		status,
		nsfw,
		viewer,
		..Default::default()
	})
}

pub fn get_chapter_list(html: Node) -> Result<Vec<Chapter>> {
	let mut chapters: Vec<Chapter> = Vec::new();
	for chapter in html.select(".chapter_list ul li a").array() {
		let obj = chapter.as_node().expect("node array");
		let url = obj.attr("href").read();
		let id = parse_incoming_url_chapter_id(url.clone());

		if let Some(id_value) = id {
			let split = id_value.split('-');
			let vec = split.collect::<Vec<&str>>();
			let chap_num = vec[vec.len() - 1].parse().unwrap();

			chapters.push(Chapter {
				id: id_value,
				chapter: chap_num,
				url,
				lang: String::from("en"),
				..Default::default()
			});
		}
	}
	Ok(chapters)
}

pub fn get_page_list(html: Node) -> Result<Vec<Page>> {
	let mut pages: Vec<Page> = Vec::new();

	for (i, page) in html.select(".panel-read-story img").array().enumerate() {
		let obj = page.as_node().expect("node array");
		let url = obj.attr("src").read();
		aidoku::prelude::println!("url: {}", url);

		pages.push(Page {
			index: i as i32,
			url,
			..Default::default()
		});
	}
	Ok(pages)
}

pub fn get_filtered_url(filters: Vec<Filter>, page: i32, url: &mut String) {
	let mut is_searching = false;
	let mut search_string = String::new();
	url.push_str("https://w.mangairo.com");

	let title_filter: Option<Filter> = filters
		.iter()
		.find(|&x| x.kind == FilterType::Title)
		.cloned();
	let author_filter: Option<Filter> = filters
		.iter()
		.find(|&x| x.kind == FilterType::Author)
		.cloned();
	let status_filter: Option<Filter> = filters
		.iter()
		.find(|&x| x.kind == FilterType::Select && x.name == "Status")
		.cloned();
	let sort_filter: Option<Filter> = filters
		.iter()
		.find(|&x| x.kind == FilterType::Select && x.name == "Sort")
		.cloned();
	let genre_filter: Option<Filter> = filters
		.iter()
		.find(|&x| x.kind == FilterType::Select && x.name == "Genre")
		.cloned();

	if let Some(title_filter_value) = title_filter {
		if let Ok(filter_value) = title_filter_value.value.as_string() {
			search_string.push_str(urlencode(filter_value.read().to_lowercase()).as_str());
			is_searching = true;
		}
	}

	if let Some(author_filter_value) = author_filter {
		if let Ok(filter_value) = author_filter_value.value.as_string() {
			if !search_string.is_empty() {
				search_string.push('_');
			}
			search_string.push_str(urlencode(filter_value.read().to_lowercase()).as_str());
			is_searching = true;
		}
	}

	if is_searching {
		url.push_str("/list/search/");
		url.push_str(&search_string);
		url.push_str("?page=");
		url.push_str(&i32_to_string(page));
	} else {
		url.push_str("/manga-list/type-");
		match sort_filter.unwrap().value.as_int().unwrap_or(-1) {
			0 => url.push_str("latest"),
			1 => url.push_str("newest"),
			2 => url.push_str("topview"),
			_ => url.push_str("latest"),
		}
		// Genre
		url.push_str("/ctg-");
		match genre_filter.unwrap().value.as_int().unwrap_or(-1) {
			0 => url.push_str("all"), // "All",
			1 => url.push('2'),       // "Action",
			2 => url.push('3'),       // "Adult",
			3 => url.push('4'),       // "Adventure",
			4 => url.push('6'),       // "Comedy",
			5 => url.push('7'),       // "Cooking",
			6 => url.push('9'),       // "Doujinshi",
			7 => url.push_str("10"),  // "Drama",
			8 => url.push_str("11"),  // "Ecchi",
			9 => url.push_str("48"),  // "Erotica",
			10 => url.push_str("12"), // "Fantasy",
			11 => url.push_str("13"), // "Gender bender",
			12 => url.push_str("14"), // "Harem",
			13 => url.push_str("15"), // "Historical",
			14 => url.push_str("16"), // "Horror",
			15 => url.push_str("45"), // "Isekai",
			16 => url.push_str("17"), // "Josei",
			17 => url.push_str("44"), // "Manhua",
			18 => url.push_str("43"), // "Manhwa",
			19 => url.push_str("19"), // "Martial arts",
			20 => url.push_str("20"), // "Mature",
			21 => url.push_str("21"), // "Mecha",
			22 => url.push_str("22"), // "Medical",
			23 => url.push_str("24"), // "Mystery",
			24 => url.push_str("25"), // "One shot",
			25 => url.push_str("47"), // "Pornographic",
			26 => url.push_str("26"), // "Phychological",
			27 => url.push_str("27"), // "Romance",
			28 => url.push_str("28"), // "School life",
			29 => url.push_str("29"), // "Sci fi",
			30 => url.push_str("30"), // "Seinen",
			31 => url.push_str("31"), // "Shoujo",
			32 => url.push_str("32"), // "Shoujo ai",
			33 => url.push_str("33"), // "Shounen",
			34 => url.push_str("34"), // "Shounen ai",
			35 => url.push_str("35"), // "Slice of Life",
			36 => url.push_str("36"), // "Smut",
			37 => url.push_str("37"), // "Sports",
			38 => url.push_str("38"), // "Supernatural",
			39 => url.push_str("39"), // "Tragedy",
			40 => url.push_str("40"), // "Webtoons",
			41 => url.push_str("41"), // "Yaoi",
			42 => url.push_str("42"), // "Yuri"
			_ => url.push_str("all"),
		}

		// State
		url.push_str("/state-");
		match status_filter.unwrap().value.as_int().unwrap_or(0) {
			0 => url.push_str("all"),
			1 => url.push_str("ongoing"),
			2 => url.push_str("completed"),
			_ => url.push_str("all"),
		}

		url.push_str("/page-");
		url.push_str(&i32_to_string(page));
	}
}

pub fn parse_incoming_url_manga_id(url: String) -> Option<String> {
	// https://chap.mangairo.com/story-pn279847
	// https://chap.mangairo.com/story-pn279847/chapter-52
	let mut parts: Vec<&str> = url.split('/').collect();
	if parts.len() >= 4 {
		parts.truncate(4);
	}

	Some(parts.join("/"))
}

pub fn parse_incoming_url_chapter_id(url: String) -> Option<String> {
	// https://chap.mangairo.com/story-pn279847/chapter-52
	let parts: Vec<&str> = url.split('/').collect();
	if parts.len() >= 4 {
		let chapter_id = parts[4];
		return Some(format!("{}", chapter_id));
	}

	None
}

// HELPER FUNCTIONS

pub fn i32_to_string(mut integer: i32) -> String {
	if integer == 0 {
		return String::from("0");
	}
	let mut string = String::with_capacity(11);
	let pos = if integer < 0 {
		string.insert(0, '-');
		1
	} else {
		0
	};
	while integer != 0 {
		let mut digit = integer % 10;
		if pos == 1 {
			digit *= -1;
		}
		string.insert(pos, char::from_u32((digit as u32) + ('0' as u32)).unwrap());
		integer /= 10;
	}
	string
}

pub fn urlencode(string: String) -> String {
	let mut result: Vec<u8> = Vec::with_capacity(string.len() * 3);
	let hex = "0123456789abcdef".as_bytes();
	let bytes = string.as_bytes();

	for byte in bytes {
		let curr = *byte;
		if curr.is_ascii_lowercase() || curr.is_ascii_uppercase() || curr.is_ascii_digit() {
			result.push(curr);
		} else {
			result.push(b'%');
			result.push(hex[curr as usize >> 4]);
			result.push(hex[curr as usize & 15]);
		}
	}

	String::from_utf8(result).unwrap_or_default()
}
