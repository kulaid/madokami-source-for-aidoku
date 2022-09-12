use aidoku::{
	std::String,
	std::{format, print, Vec},
};
use md5::{Digest, Md5};

pub fn urlencode(string: String) -> String {
	let mut result: Vec<u8> = Vec::with_capacity(string.len() * 3);
	let hex = "0123456789ABCDEF".as_bytes();
	let bytes = string.as_bytes();

	for byte in bytes {
		let curr = *byte;
		if curr.is_ascii_alphanumeric() || curr == 45 || curr == 95 {
			// 45: -
			// 95: _
			result.push(curr);
		} else {
			result.push(b'%');
			result.push(hex[curr as usize >> 4]);
			result.push(hex[curr as usize & 15]);
		}
	}

	String::from_utf8(result).unwrap_or_default()
}

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

const HEX_CHARS: [char; 16] = [
	'0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f',
];

pub fn md5(str: &String) -> String {
	let mut hasher = Md5::default();
	hasher.update(str);

	let bytes = hasher.finalize();
	let mut result = String::new();

	for b in bytes {
		let x = ((b >> 4) & 0xf) as usize;
		let y = (b & 0xf) as usize;
		result.push(HEX_CHARS[x]);
		result.push(HEX_CHARS[y]);
	}

	return result;
}

const GSN_KEY: &str = "4e0a48e1c0b54041bce9c8f0e036124d";

pub fn generate_gsn_hash(args: &mut Vec<(String, String)>) -> String {
	let mut temp = String::new();

	args.sort_by(|a, b| a.0.cmp(&b.0));

	temp.push_str(GSN_KEY);
	temp.push_str("GET");

	for a in args {
		temp.push_str(&a.0);
		temp.push_str(&urlencode(String::from(&a.1)));
	}

	temp.push_str(GSN_KEY);

	print(&temp);

	return md5(&temp);
}

pub fn generate_get_query(args: &mut Vec<(String, String)>) -> String {
	args.push((String::from("gak"), String::from("android_manhuaren2")));

	let gsn = generate_gsn_hash(args);

	args.push((String::from("gsn"), String::from(&gsn)));

	print("gsn:");
	print(gsn);

	let mut qs = String::new();

	for (i, a) in args.iter().enumerate() {
		if i > 0 {
			qs.push_str("&");
		}

		let v = urlencode(String::from(&a.1));
		qs.push_str(&format(format_args!("{}={}", a.0, v)));
	}

	return qs;
}
