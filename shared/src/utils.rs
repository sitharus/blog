use std::str::FromStr;

use anyhow::anyhow;
use askama::Template;
use cgi;
use chrono::{DateTime, Datelike, Month, Utc};
use num_traits::FromPrimitive;
use serde::Deserialize;
use serde_querystring::{ParseMode, from_bytes, from_str};

pub fn post_body<T: for<'a> Deserialize<'a>>(request: &cgi::Request) -> anyhow::Result<T> {
    let body = request.body();
    let result = from_bytes(body, ParseMode::Duplicate);
    result.map_err(|e| anyhow!(e))
}

pub fn render_html<T: Template>(template: T) -> anyhow::Result<cgi::Response> {
    render_html_status(200, template)
}

pub fn render_html_status<S, T: Template>(status: S, template: T) -> anyhow::Result<cgi::Response>
where
    cgi::http::StatusCode: TryFrom<S>,
    <cgi::http::StatusCode as TryFrom<S>>::Error: Into<cgi::http::Error>,
{
    let content = template.render()?;
    Ok(cgi::html_response(status, content))
}

pub fn parse_query_string<T: for<'a> Deserialize<'a>>(query_string: &str) -> anyhow::Result<T> {
    from_str(query_string, ParseMode::UrlEncoded).map_err(|e| anyhow!(e))
}

pub fn parse_into<T: FromStr>(s: &str) -> anyhow::Result<T> {
    s.parse().map_err(|_| anyhow!("Failed to parse string"))
}

pub fn render_redirect(action: &str, site_id: i32) -> anyhow::Result<cgi::Response> {
    let body: Vec<u8> = "Redirecting".as_bytes().to_vec();
    let response = cgi::http::response::Builder::new()
        .status(302)
        .header(
            cgi::http::header::LOCATION,
            format!("?action={}&site={}", action, site_id),
        )
        .body(body)?;
    Ok(response)
}

pub trait BlogUtils {
    fn parse_into<T: FromStr>(&self) -> anyhow::Result<T>;
}

impl BlogUtils for String {
    fn parse_into<T: FromStr>(&self) -> anyhow::Result<T> {
        self.parse().map_err(|_| anyhow!("Failed to parse string"))
    }
}

impl BlogUtils for str {
    fn parse_into<T: FromStr>(&self) -> anyhow::Result<T> {
        self.parse().map_err(|_| anyhow!("Failed to parse string"))
    }
}

impl BlogUtils for Option<&String> {
    fn parse_into<T: FromStr>(&self) -> anyhow::Result<T> {
        self.ok_or(anyhow!("String was none"))?
            .parse()
            .map_err(|_| anyhow!("Failed to parse string"))
    }
}

pub fn blog_post_url(
    slug: String,
    post_date: DateTime<Utc>,
    timezone: chrono_tz::Tz,
    base_url: String,
) -> ::askama::Result<String> {
    let local_date = post_date.with_timezone(&timezone);
    let month = Month::from_u32(local_date.month())
        .ok_or(::askama::Error::Custom("Could not find month".into()))?
        .name();
    let url = format!("{}{}/{}/{}.html", base_url, local_date.year(), month, slug);
    Ok(url)
}
