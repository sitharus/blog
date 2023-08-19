use std::collections::HashMap;

use crate::utils::blog_post_url;

use super::{CommonData, HydratedPost};
use chrono::{offset::Utc, DateTime, Datelike, Month, NaiveDate};
use chrono_tz::Tz;
use num_traits::FromPrimitive;
use ordinal::Ordinal;
use pulldown_cmark::{Event, Options, Parser, Tag};

pub fn posturl(post: &HydratedPost, common: &CommonData) -> ::askama::Result<String> {
    blog_post_url(
        post.url_slug.clone(),
        post.post_date,
        common.base_url.clone(),
    )
}

pub fn month_name(month: u32) -> ::askama::Result<String> {
    let month = Month::from_u32(month)
        .ok_or(::askama::Error::Custom("Could not find month".into()))?
        .name();
    Ok(String::from(month))
}

pub fn format_human_date(date_time: &NaiveDate) -> ::askama::Result<String> {
    Ok(date_time.format("%A, %-d %B, %C%y").to_string())
}

pub fn format_human_datetime(date_time: &DateTime<Utc>, timezone: &Tz) -> ::askama::Result<String> {
    Ok(date_time
        .with_timezone(timezone)
        .format("%A, %-d %B, %C%y at %-I:%m%P %Z")
        .to_string())
}

pub fn format_rfc3339_datetime(date_time: &DateTime<Utc>) -> ::askama::Result<String> {
    Ok(date_time.to_rfc3339())
}

pub fn format_rfc2822_datetime(date_time: &DateTime<Utc>) -> ::askama::Result<String> {
    Ok(date_time.to_rfc2822())
}

pub fn format_rfc3339_date(date: &NaiveDate) -> ::askama::Result<String> {
    date.and_hms_opt(0, 0, 0)
        .ok_or(::askama::Error::Custom(
            "Could not find midnight UTC".into(),
        ))?
        .and_local_timezone(Utc)
        .earliest()
        .ok_or(::askama::Error::Custom("Cannot convert to UTC".into()))
        .map(|d| d.to_rfc3339())
}

pub fn format_rfc2822_date(date: &NaiveDate) -> ::askama::Result<String> {
    date.and_hms_opt(0, 0, 0)
        .ok_or(::askama::Error::Custom(
            "Could not find midnight UTC".into(),
        ))?
        .and_local_timezone(Utc)
        .earliest()
        .ok_or(::askama::Error::Custom("Cannot convert to UTC".into()))
        .map(|d| d.to_rfc2822())
}

pub fn pluralise(base: &str, count: &Option<i64>) -> ::askama::Result<String> {
    match count {
        Some(1) => Ok(base.to_string()),
        _ => Ok(format!("{}s", base)),
    }
}

pub fn format_weekday(date: &NaiveDate) -> ::askama::Result<String> {
    let weekday = date.weekday();
    let day = Ordinal(date.day());
    Ok(format!("{} {}", weekday, day))
}

pub fn format_markdown<S>(
    content: S,
    common: &CommonData,
    before_cut: bool,
) -> ::askama::Result<String>
where
    S: AsRef<str>,
{
    let mut current_image: String = String::new();
    let mut in_image = false;
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_SMART_PUNCTUATION);
    options.insert(Options::ENABLE_FOOTNOTES);
    let parser = Parser::new_ext(content.as_ref(), options).map(|event| match &event {
        /*
        An image is represented as three events: Start(Tag::Image) Text(alt text) End(Tag::Image)
        So I split the alt attribute between start and end. Fortunately I only need to do
        the image lookup once.
        */
        Event::Start(tag) => match tag {
            Tag::Image(_link_type, destination, title) if destination.starts_with("!!") => {
                let dest_args: Vec<&str> = destination.split("?").collect();
                let image_id: i32 = dest_args[0][2..].parse().unwrap();
                let args: HashMap<String, String> = if dest_args.len() > 1 {
                    serde_querystring::from_str(
                        dest_args[1],
                        serde_querystring::ParseMode::UrlEncoded,
                    )
                    .unwrap()
                } else {
                    HashMap::new()
                };

                let image = common.media.get(&image_id).unwrap();
                let dest = format!("{}{}", common.media_base_url, image.metadata.fullsize_name);
                let mut html = String::new();

                html.push_str("<picture>");
                html.push_str(&format!(
                    r#"<source srcset="{}{}" type="{}">"#,
                    common.media_base_url,
                    image.metadata.fullsize_name,
                    image.metadata.content_type
                ));

                in_image = true;
                current_image.push_str(&format!(r#"<img src="{}" title="{}""#, dest, title));

                if let Some(a) = args.get("class") {
                    current_image.push_str(&format!(r#" class="{}" "#, a));
                }

                Event::Text("".into())
            }
            _ => event,
        },
        Event::Text(txt) if in_image => {
            current_image.push_str(&format!(r#" alt="{}" "#, txt));
            Event::Text("".into())
        }
        Event::End(tag) => match tag {
            Tag::Image(_link_type, destination, _title) if destination.starts_with("!!") => {
                current_image.push_str("></picture>");
                let tag = current_image.clone();
                current_image = String::new();
                in_image = false;
                Event::Html(tag.into())
            }
            _ => event,
        },
        _ => event,
    });
    let mut html_output = String::new();

    match before_cut {
        true => pulldown_cmark::html::push_html(
            &mut html_output,
            parser.take_while(|e| match e {
                Event::Html(node) if node.starts_with("<blog-cut>") => false,
                _ => true,
            }),
        ),
        false => pulldown_cmark::html::push_html(&mut html_output, parser),
    };

    Ok(html_output)
}
