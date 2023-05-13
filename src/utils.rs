use std::str::FromStr;

use cgi;
use serde_querystring::{from_bytes, ParseMode, from_str};
use serde::Deserialize;
use anyhow::anyhow;
use askama::Template;

pub fn post_body<T: for<'a> Deserialize<'a>>(request: &cgi::Request) -> anyhow::Result<T> {
	let body = request.body();
	let result = from_bytes(body, ParseMode::UrlEncoded);
	return result.map_err(|e| anyhow!(e))
}

pub fn render_html<T:  Template>(template: T) -> anyhow::Result<cgi::Response> {
	let content = template.render()?;
	Ok(cgi::html_response(200, content))
}

pub fn parse_query_string<T: for<'a> Deserialize<'a>>(query_string: &str) -> anyhow::Result<T> {
    from_str(query_string, ParseMode::UrlEncoded).map_err(|e| anyhow!(e))
}

pub fn parse_into<T: FromStr>(s: &String) -> anyhow::Result<T> {
	s.parse().map_err(|_| anyhow!("Failed to parse string"))
}

pub fn render_redirect(action: &str) -> anyhow::Result<cgi::Response> {
	let body: Vec<u8> = "Redirecting".as_bytes().to_vec();
	let response = http::response::Builder::new()
		.status(302)
		.header(http::header::LOCATION, format!("?action={}", action))
		.body(body)?;
	Ok(response)
}
