use crate::url::Url;
use aidoku::{
	error::Result,
	std::{html::Node, json, ArrayRef, String, ValueRef, Vec},
	Manga, MangaPageResult, MangaStatus,
};
use alloc::string::ToString;

pub trait MangaListResponse {
	fn get_page_result(self) -> Result<MangaPageResult>;
}

impl MangaListResponse for Node {
	fn get_page_result(self) -> Result<MangaPageResult> {
		let manga_list_str = self
			.select("div.exemptComic-box")
			.attr("list")
			.read()
			.split('"')
			.enumerate()
			.map(|(index, str)| {
				if index % 2 == 0 {
					str.replace('\'', "\"")
				} else {
					str.to_string()
				}
			})
			.collect::<Vec<_>>()
			.join("\"");
		let manga = json::parse(manga_list_str)?.as_array()?.get_manga_list()?;

		let has_more = !self.select("li.page-all-item").last().has_class("active");

		Ok(MangaPageResult { manga, has_more })
	}
}

impl MangaListResponse for ValueRef {
	fn get_page_result(self) -> Result<MangaPageResult> {
		let results_obj = self.as_object()?.get("results").as_object()?;

		let manga = results_obj.get("list").as_array()?.get_manga_list()?;

		let total = results_obj.get("total").as_int()?;
		let limit = results_obj.get("limit").as_int()?;
		let offset = results_obj.get("offset").as_int()?;
		let has_more = (offset + limit) < total;

		Ok(MangaPageResult { manga, has_more })
	}
}

trait MangaArr {
	fn get_manga_list(self) -> Result<Vec<Manga>>;
}

impl MangaArr for ArrayRef {
	fn get_manga_list(self) -> Result<Vec<Manga>> {
		let mut manga = Vec::<Manga>::new();
		for manga_value in self {
			let manga_obj = manga_value.as_object()?;

			let manga_id = manga_obj.get("path_word").as_string()?.read();

			let cover = manga_obj
				.get("cover")
				.as_string()?
				.read()
				.replace(".328x422.jpg", "");

			let title = manga_obj.get("name").as_string()?.read();

			let artist = manga_obj
				.get("author")
				.as_array()?
				.filter_map(|value| value.as_object().ok())
				.filter_map(|obj| obj.get("name").as_string().ok())
				.map(|str_ref| str_ref.read())
				.collect::<Vec<_>>()
				.join("、");

			let manga_url = Url::Manga(&manga_id).to_string();

			let status_code = manga_obj.get("status").as_int().unwrap_or(-1);
			let status = match status_code {
				0 => MangaStatus::Ongoing,
				1 | 2 => MangaStatus::Completed,
				_ => MangaStatus::Unknown,
			};

			manga.push(Manga {
				id: manga_id,
				cover,
				title,
				author: artist.clone(),
				artist,
				url: manga_url,
				status,
				..Default::default()
			});
		}

		Ok(manga)
	}
}

pub trait NodeArrValue {
	fn ok_text(self) -> Option<String>;
}

impl NodeArrValue for ValueRef {
	fn ok_text(self) -> Option<String> {
		self.as_node().map(|node| node.text().read()).ok()
	}
}
