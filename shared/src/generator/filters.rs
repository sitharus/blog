use super::{CommonData, HydratedPost};
use chrono::{offset::Utc, DateTime, Datelike, Month, NaiveDate};
use num_traits::FromPrimitive;
use ordinal::Ordinal;
use pulldown_cmark::{Event, Tag};

pub fn posturl(post: &HydratedPost, common: &CommonData) -> ::askama::Result<String> {
    let month = Month::from_u32(post.post_date.month())
        .ok_or(::askama::Error::Custom("Could not find month".into()))?
        .name();
    let url = format!(
        "{}{}/{}/{}.html",
        common.base_url,
        post.post_date.year(),
        month,
        post.url_slug
    );
    Ok(url)
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

pub fn format_human_datetime(date_time: &DateTime<Utc>) -> ::askama::Result<String> {
    Ok(date_time.format("%A, %-d %B, %C%y at %-I:%m%P").to_string())
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

pub fn format_markdown<S>(content: S, common: &CommonData) -> ::askama::Result<String>
where
    S: AsRef<str>,
{
    let parser = pulldown_cmark::Parser::new(content.as_ref()).map(|event| match &event {
        /*
        An image is represented as three events: Start(Tag::Image) Text(alt text) End(Tag::Image)
        So I split the alt attribute between start and end. Fortunately I only need to do
        the image lookup once.
        */
        Event::Start(tag) => match tag {
            Tag::Image(_link_type, destination, title) if destination.starts_with("!!") => {
                let image_id: i32 = destination[2..].parse().unwrap();
                let image = common.media.get(&image_id).unwrap();
                let dest = format!("{}{}", common.media_base_url, image.metadata.fullsize_name);
                let mut html = String::new();

                html.push_str("<picture>");
                html.push_str(&format!(
                    r#"<source srcset="{}" type="{}">"#,
                    image.metadata.fullsize_name, image.metadata.content_type
                ));

                html.push_str(&format!(r#"<img src="{}" title="{}" alt=""#, dest, title));

                Event::Html(html.into())
            }
            _ => event,
        },
        Event::End(tag) => match tag {
            Tag::Image(_link_type, destination, _title) if destination.starts_with("!!") => {
                Event::Html(r#""></picture>"#.into())
            }
            _ => event,
        },
        _ => event,
    });

    let mut html_output = String::new();
    pulldown_cmark::html::push_html(&mut html_output, parser);
    Ok(html_output)
}
