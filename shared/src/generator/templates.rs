use std::collections::HashMap;

use chrono::{offset::Utc, DateTime, Datelike, Month, NaiveDate};
use chrono_tz::Tz;
use latex2mathml::{latex_to_mathml, DisplayStyle};
use num_traits::FromPrimitive;
use ordinal::Ordinal;
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag};
use serde_json::{from_value, Value};
use sqlx::PgPool;
use tera::{Filter, Tera};

use crate::types::{CommonData, HydratedPost, Media};

pub static BASE: &str = include_str!("../../../templates/generated/tera_base.html");
pub static MACROS: &str = include_str!("../../../templates/generated/tera_macros.html");
pub static POST: &str = include_str!("../../../templates/generated/post.html");
pub static INDEX: &str = include_str!("../../../templates/generated/index.html");
pub static MONTH_INDEX: &str = include_str!("../../../templates/generated/month_index.html");
pub static YEAR_INDEX: &str = include_str!("../../../templates/generated/year_index.html");
pub static RSS: &str = include_str!("../../../templates/generated/feed.xml");
pub static ATOM: &str = include_str!("../../../templates/generated/atom.xml");

pub struct TemplateInfo {
    pub custom_path: Option<String>,
    pub can_customise_path: bool,
    pub contents: String,
}

impl TemplateInfo {
    fn create_from_default(contents: String) -> TemplateInfo {
        TemplateInfo {
            custom_path: None,
            can_customise_path: false,
            contents,
        }
    }
}

static TEMPLATE_MAP: [(&str, &str, &str); 8] = [
    ("base", "base.html", BASE),
    ("macros", "macros.html", MACROS),
    ("posts", "post.html", POST),
    ("index", "index.html", INDEX),
    ("month_index", "month_index.html", MONTH_INDEX),
    ("year_index", "year_index.html", YEAR_INDEX),
    ("atom", "atom.xml", ATOM),
    ("rss", "rss.xml", RSS),
];

pub fn default_templates() -> HashMap<String, TemplateInfo> {
    let mut templates = HashMap::new();
    for (name, _, template) in TEMPLATE_MAP {
        templates.insert(
            name.into(),
            TemplateInfo::create_from_default(template.into()),
        );
    }
    return templates;
}

pub async fn load_templates(
    database: &PgPool,
    site_id: i32,
    common: &CommonData,
) -> anyhow::Result<Tera> {
    let templates = sqlx::query!(
        "SELECT template_kind, content FROM templates WHERE site_id=$1",
        site_id
    )
    .fetch_all(database)
    .await?;
    let lookup: HashMap<String, String> = templates
        .into_iter()
        .map(|r| (r.template_kind, r.content))
        .collect();

    let mut tera = Tera::default();

    for (name, filename, default_template) in TEMPLATE_MAP {
        tera.add_raw_template(
            filename,
            lookup
                .get(name)
                .map(|c| c.as_str())
                .unwrap_or(default_template),
        )?;
    }

    let base_url = common.base_url.clone();
    let tz = common.timezone.clone();
    let media_base_url = common.media_base_url.clone();
    let media = common.media.clone();
    let post_url = BuildUrl::new(base_url);
    let static_url = BuildUrl::new(common.static_base_url.clone());

    tera.register_filter("posturl", post_url);
    tera.register_filter("staticurl", static_url);
    tera.register_filter("format_rfc3339_date", format_rfc3339_date);
    tera.register_filter("format_rfc3339_datetime", format_rfc3339_datetime);
    tera.register_filter("format_rfc2822_date", format_rfc2822_date);
    tera.register_filter("format_rfc2822_datetime", format_rfc2822_datetime);
    tera.register_filter("month_name", month_name);
    tera.register_filter("format_human_date", format_human_date);
    tera.register_filter("format_weekday", format_weekday);
    tera.register_filter("year", year);
    tera.register_filter(
        "format_human_datetime",
        move |v: &Value, _args: &HashMap<String, Value>| format_human_datetime(v, &tz),
    );
    tera.register_filter(
        "format_markdown",
        move |v: &Value, args: &HashMap<String, Value>| {
            format_markdown(v, args, &media_base_url, &media)
        },
    );

    return Ok(tera);
}

pub fn blog_post_url(
    slug: String,
    post_date: chrono::NaiveDate,
    base_url: String,
) -> tera::Result<String> {
    let month = Month::from_u32(post_date.month())
        .ok_or(tera::Error::msg("Could not find month".to_string()))?
        .name();
    let url = format!("{}{}/{}/{}.html", base_url, post_date.year(), month, slug);
    Ok(url)
}

struct BuildUrl {
    base_url: String,
}

impl BuildUrl {
    pub fn new(base_url: String) -> Self {
        BuildUrl { base_url }
    }
}

impl Filter for BuildUrl {
    fn filter(&self, post: &Value, args: &HashMap<String, Value>) -> tera::Result<Value> {
        match post {
            Value::Object(_) => {
                let target: Option<String> = args
                    .get("target")
                    .and_then(|t| from_value::<String>(t.clone()).ok());
                let post: HydratedPost =
                    serde_json::from_value(post.clone()).map_err(tera::Error::from)?;

                let url = match target.as_deref() {
                    Some("year") => Ok(format!("{}{}/", self.base_url, post.post_date.year())),
                    Some("month") => Ok(format!(
                        "{}{}/{}/",
                        self.base_url,
                        post.post_date.year(),
                        post.post_date.format("%B")
                    )),
                    _ => {
                        blog_post_url(post.url_slug.clone(), post.post_date, self.base_url.clone())
                    }
                }?;
                Ok(Value::String(url.into()))
            }
            Value::String(s) => Ok(Value::String(format!("{}{}", self.base_url, s))),
            _ => Err(tera::Error::msg(format!(
                "Not able to build a URL from {:?}",
                post
            ))),
        }
    }

    fn is_safe(&self) -> bool {
        true
    }
}

fn format_rfc3339_date(in_date: &Value, _args: &HashMap<String, Value>) -> ::tera::Result<Value> {
    let date: NaiveDate = from_value(in_date.clone()).map_err(tera::Error::from)?;

    date.and_hms_opt(0, 0, 0)
        .ok_or(tera::Error::msg("Could not find midnight UTC"))?
        .and_local_timezone(Utc)
        .earliest()
        .ok_or(tera::Error::msg("Cannot convert to UTC"))
        .map(|d| d.to_rfc3339())
        .map(Value::String)
}

pub fn format_rfc3339_datetime(
    in_datetime: &Value,
    _args: &HashMap<String, Value>,
) -> tera::Result<Value> {
    from_value::<DateTime<Utc>>(in_datetime.clone())
        .map(|d| d.to_rfc3339())
        .map_err(tera::Error::from)
        .map(Value::String)
}

fn format_rfc2822_date(in_date: &Value, _args: &HashMap<String, Value>) -> ::tera::Result<Value> {
    let date: NaiveDate = from_value(in_date.clone()).map_err(tera::Error::from)?;

    date.and_hms_opt(0, 0, 0)
        .ok_or(tera::Error::msg("Could not find midnight UTC"))?
        .and_local_timezone(Utc)
        .earliest()
        .ok_or(tera::Error::msg("Cannot convert to UTC"))
        .map(|d| d.to_rfc2822())
        .map(Value::String)
}

pub fn format_rfc2822_datetime(
    in_datetime: &Value,
    _args: &HashMap<String, Value>,
) -> tera::Result<Value> {
    from_value::<DateTime<Utc>>(in_datetime.clone())
        .map(|d| d.to_rfc2822())
        .map_err(tera::Error::from)
        .map(Value::String)
}

fn month_name(in_date_time: &Value, _args: &HashMap<String, Value>) -> tera::Result<Value> {
    match in_date_time {
        Value::Number(_) => from_value::<i32>(in_date_time.clone())
            .map_err(tera::Error::from)
            .and_then(|n| Month::from_i32(n).ok_or(tera::Error::msg("Not a month")))
            .map(|n| n.name())
            .map(|s| Value::String(s.into())),
        Value::Object(_) | Value::String(_) => from_value::<NaiveDate>(in_date_time.clone())
            .map_err(tera::Error::from)
            .map(|m| m.format("%B").to_string())
            .map(|s| Value::String(s.into())),

        _ => Err(tera::Error::msg(format!(
            "Not formattable as month {:?}",
            in_date_time
        ))),
    }
}

fn year(in_date_time: &Value, _args: &HashMap<String, Value>) -> tera::Result<Value> {
    from_value::<NaiveDate>(in_date_time.clone())
        .map_err(tera::Error::from)
        .map(|m| m.year())
        .map(|s| Value::Number(s.into()))
}

fn format_human_date(in_date_time: &Value, _args: &HashMap<String, Value>) -> tera::Result<Value> {
    from_value::<NaiveDate>(in_date_time.clone())
        .map_err(tera::Error::from)
        .map(|date_time| date_time.format("%A, %-d %B, %C%y").to_string())
        .map(|s| Value::String(s.into()))
}

fn format_human_datetime(in_date_time: &Value, tz: &Tz) -> tera::Result<Value> {
    from_value::<DateTime<Utc>>(in_date_time.clone())
        .map_err(tera::Error::from)
        .map(|date_time| {
            date_time
                .with_timezone(tz)
                .format("%A, %-d %B, %C%y at %-I:%m%P %Z")
                .to_string()
        })
        .map(|s| Value::String(s.into()))
}

fn format_weekday(in_date: &Value, _args: &HashMap<String, Value>) -> tera::Result<Value> {
    from_value::<NaiveDate>(in_date.clone())
        .map(|date| format!("{} {}", date.weekday(), Ordinal(date.day())))
        .map(Value::String)
        .map_err(tera::Error::from)
}
/*

pub fn format_weekday(date: &NaiveDate) -> ::askama::Result<String> {
    let weekday = date.weekday();
    let day = Ordinal(date.day());
    Ok(format!("{} {}", weekday, day))
}*/
fn format_markdown(
    value: &Value,
    args: &HashMap<String, Value>,
    media_base_url: &String,
    media: &HashMap<i32, Media>,
) -> tera::Result<Value> {
    let content: String = from_value(value.clone()).map_err(tera::Error::from)?;
    let before_cut = args
        .get("before_cut")
        .and_then(|v| from_value::<bool>(v.clone()).ok())
        .unwrap_or(false);
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

                let image = media.get(&image_id).unwrap();
                let dest = format!("{}{}", media_base_url, image.metadata.fullsize_name);
                let mut html = String::new();

                html.push_str("<picture>");
                html.push_str(&format!(
                    r#"<source srcset="{}{}" type="{}">"#,
                    media_base_url, image.metadata.fullsize_name, image.metadata.content_type
                ));

                in_image = true;
                current_image.push_str(&format!(r#"<img src="{}" title="{}""#, dest, title));

                if let Some(a) = args.get("class") {
                    current_image.push_str(&format!(r#" class="{}" "#, a));
                }

                Event::Text("".into())
            }
            Tag::CodeBlock(CodeBlockKind::Fenced(lang)) if !lang.is_empty() => Event::Html(
                format!(
                    "<pre class=\"fenced-code language-{}\"><code class=\"language-{}\">",
                    lang, lang
                )
                .into(),
            ),
            _ => event,
        },
        Event::Code(code) if code.starts_with("$$") && code.ends_with("$$") => {
            let mathml = latex_to_mathml(code[2..(code.len() - 2)].into(), DisplayStyle::Inline)
                .unwrap_or("Bad Math!".into());
            Event::Html(mathml.into())
        }
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
            Tag::CodeBlock(CodeBlockKind::Fenced(lang)) if !lang.is_empty() => {
                Event::Html("</code></pre>".into())
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

    Ok(Value::String(html_output))
}
