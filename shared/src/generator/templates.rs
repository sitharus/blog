use std::collections::HashMap;

use chrono::{DateTime, Datelike, Month, NaiveDate, offset::Utc};
use chrono_tz::Tz;
use latex2mathml::{DisplayStyle, latex_to_mathml};
use num_traits::FromPrimitive;
use ordinal::Ordinal;
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use serde_json::{Value, from_value};
use sqlx::PgPool;
use tera::{Filter, Function, Tera};

use crate::types::{CommonData, HydratedPost, Media};

pub static BASE: &str = include_str!("../../../templates/generated/tera_base.html");
pub static MACROS: &str = include_str!("../../../templates/generated/tera_macros.html");
pub static POST: &str = include_str!("../../../templates/generated/post.html");
pub static INDEX: &str = include_str!("../../../templates/generated/index.html");
pub static MONTH_INDEX: &str = include_str!("../../../templates/generated/month_index.html");
pub static YEAR_INDEX: &str = include_str!("../../../templates/generated/year_index.html");
pub static RSS: &str = include_str!("../../../templates/generated/feed.xml");
pub static ATOM: &str = include_str!("../../../templates/generated/atom.xml");
pub static CSS: &str = include_str!("../../../templates/generated/blog.css");

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

static TEMPLATE_MAP: [(&str, &str, &str); 9] = [
    ("base", "base.html", BASE),
    ("macros", "macros.html", MACROS),
    ("posts", "post.html", POST),
    ("index", "index.html", INDEX),
    ("month_index", "month_index.html", MONTH_INDEX),
    ("year_index", "year_index.html", YEAR_INDEX),
    ("atom", "atom.xml", ATOM),
    ("rss", "rss.xml", RSS),
    ("css", "blog.css", CSS),
];

pub fn default_templates() -> HashMap<String, TemplateInfo> {
    let mut templates = HashMap::new();
    for (name, _, template) in TEMPLATE_MAP {
        templates.insert(
            name.into(),
            TemplateInfo::create_from_default(template.into()),
        );
    }
    templates
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
    let tz = common.timezone;
    let media_base_url = common.media_base_url.clone();
    let media = common.media.clone();
    let post_url = BuildUrl::new(base_url.clone(), tz);
    let site_url = BuildUrl::new(base_url, tz);
    let static_url = BuildUrl::new(common.static_base_url.clone(), tz);

    tera.register_filter("posturl", post_url);
    tera.register_filter("staticurl", static_url);
    tera.register_filter("format_rfc3339_date", RFC3339Format::new(tz));
    tera.register_filter("format_rfc3339_datetime", format_rfc3339_datetime);
    tera.register_filter("format_rfc2822_date", RFC2822Format::new(tz));
    tera.register_filter("format_rfc2822_datetime", format_rfc2822_datetime);
    tera.register_filter("month_name", MonthNameFormat::new(tz));
    tera.register_filter("format_human_date", HumanDateFormat::new(tz));
    tera.register_filter("format_weekday", WeekDayFormat::new(tz));
    tera.register_filter("year", YearFormat::new(tz));
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

    tera.register_function("buildurl", site_url);
    tera.register_tester("cut", has_cut);

    Ok(tera)
}

fn has_cut(value: Option<&Value>, _args: &[Value]) -> tera::Result<bool> {
    match value {
        Some(v) => Ok(from_value::<String>(v.clone())?.contains("<blog-cut>")),
        None => Ok(false),
    }
}

pub fn blog_post_url(
    slug: String,
    post_date: chrono::DateTime<Utc>,
    timezone: chrono_tz::Tz,
    base_url: String,
) -> tera::Result<String> {
    let local_date = post_date.with_timezone(&timezone);
    let month = Month::from_u32(local_date.month())
        .ok_or(tera::Error::msg("Could not find month".to_string()))?
        .name();
    let url = format!("{}{}/{}/{}.html", base_url, local_date.year(), month, slug);
    Ok(url)
}

struct BuildUrl {
    base_url: String,
    timezone: chrono_tz::Tz,
}

impl BuildUrl {
    pub fn new(base_url: String, timezone: chrono_tz::Tz) -> Self {
        BuildUrl { base_url, timezone }
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
                    _ => blog_post_url(
                        post.url_slug.clone(),
                        post.post_date,
                        self.timezone,
                        self.base_url.clone(),
                    ),
                }?;
                Ok(Value::String(url))
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

impl Function for BuildUrl {
    fn call(&self, args: &HashMap<String, Value>) -> tera::Result<Value> {
        if let Some(static_url) = args.get("static") {
            Ok(Value::String(format!("{}/{}", self.base_url, static_url)))
        } else if args.get("home").is_some() {
            Ok(Value::String(format!("{}/", self.base_url)))
        } else if let Some(page) = args.get("page") {
            match page {
                Value::Object(page_obj) => match page_obj.get("url_slug") {
                    Some(slug) => Ok(Value::String(format!("{}/{}", self.base_url, slug))),
                    _ => Err(tera::Error::msg("Not a page")),
                },
                _ => Err(tera::Error::msg("Not a page")),
            }
        } else {
            Err(tera::Error::msg("Unsupported URL request"))
        }
    }

    fn is_safe(&self) -> bool {
        false
    }
}

struct RFC3339Format {
    timezone: chrono_tz::Tz,
}

impl RFC3339Format {
    pub fn new(timezone: chrono_tz::Tz) -> Self {
        Self { timezone }
    }
}

impl Filter for RFC3339Format {
    fn filter(&self, in_date: &Value, _args: &HashMap<String, Value>) -> ::tera::Result<Value> {
        if let Ok(date) = from_value::<NaiveDate>(in_date.clone()) {
            date.and_hms_opt(0, 0, 0)
                .ok_or(tera::Error::msg("Could not find midnight UTC"))?
                .and_local_timezone(self.timezone)
                .earliest()
                .ok_or(tera::Error::msg("Cannot convert to UTC"))
                .map(|d| d.to_rfc3339())
                .map(Value::String)
        } else if let Ok(date) = from_value::<DateTime<Utc>>(in_date.clone()) {
            Ok(Value::String(date.to_rfc3339()))
        } else {
            Err(tera::Error::msg(format!(
                "Cannot format rfc3339 from {}",
                in_date
            )))
        }
    }

    fn is_safe(&self) -> bool {
        true
    }
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

struct RFC2822Format {
    timezone: chrono_tz::Tz,
}

impl RFC2822Format {
    pub fn new(timezone: chrono_tz::Tz) -> Self {
        Self { timezone }
    }
}
impl Filter for RFC2822Format {
    fn filter(&self, in_date: &Value, _args: &HashMap<String, Value>) -> ::tera::Result<Value> {
        if let Ok(date) = from_value::<NaiveDate>(in_date.clone()) {
            date.and_hms_opt(0, 0, 0)
                .ok_or(tera::Error::msg("Could not find midnight UTC"))?
                .and_local_timezone(self.timezone)
                .earliest()
                .ok_or(tera::Error::msg("Cannot convert to UTC"))
                .map(|d| d.to_rfc2822())
                .map(Value::String)
        } else if let Ok(date) = from_value::<DateTime<Utc>>(in_date.clone()) {
            Ok(Value::String(date.to_rfc2822()))
        } else {
            Err(tera::Error::msg(format!(
                "Cannot format rfc2822 from {}",
                in_date
            )))
        }
    }

    fn is_safe(&self) -> bool {
        true
    }
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

struct MonthNameFormat {
    timezone: chrono_tz::Tz,
}

impl MonthNameFormat {
    pub fn new(timezone: chrono_tz::Tz) -> Self {
        MonthNameFormat { timezone }
    }
}

impl Filter for MonthNameFormat {
    fn filter(&self, in_date_time: &Value, _args: &HashMap<String, Value>) -> tera::Result<Value> {
        match in_date_time {
            Value::Number(_) => from_value::<i32>(in_date_time.clone())
                .map_err(tera::Error::from)
                .and_then(|n| Month::from_i32(n).ok_or(tera::Error::msg("Not a month")))
                .map(|n| n.name())
                .map(|s| Value::String(s.into())),
            Value::Object(_) | Value::String(_) => {
                if let Ok(date) = from_value::<NaiveDate>(in_date_time.clone()) {
                    Ok(Value::String(date.format("%B").to_string()))
                } else if let Ok(date) = from_value::<DateTime<Utc>>(in_date_time.clone()) {
                    Ok(Value::String(
                        date.with_timezone(&self.timezone).format("%B").to_string(),
                    ))
                } else {
                    Err(tera::Error::msg(format!(
                        "Could not extract month from {}",
                        in_date_time
                    )))
                }
            }
            _ => Err(tera::Error::msg(format!(
                "Not formattable as month {:?}",
                in_date_time
            ))),
        }
    }

    fn is_safe(&self) -> bool {
        true
    }
}

struct YearFormat {
    timezone: chrono_tz::Tz,
}

impl YearFormat {
    pub fn new(timezone: chrono_tz::Tz) -> Self {
        YearFormat { timezone }
    }
}

impl Filter for YearFormat {
    fn filter(&self, in_date_time: &Value, _args: &HashMap<String, Value>) -> tera::Result<Value> {
        if let Ok(date) = from_value::<NaiveDate>(in_date_time.clone()) {
            Ok(Value::Number(date.year().into()))
        } else if let Ok(date) = from_value::<DateTime<Utc>>(in_date_time.clone()) {
            Ok(Value::Number(
                date.with_timezone(&self.timezone).year().into(),
            ))
        } else {
            Err(tera::Error::msg(format!(
                "Could not format year from {}",
                in_date_time.clone()
            )))
        }
    }

    fn is_safe(&self) -> bool {
        true
    }
}

struct HumanDateFormat {
    timezone: chrono_tz::Tz,
}

impl HumanDateFormat {
    pub fn new(timezone: chrono_tz::Tz) -> Self {
        HumanDateFormat { timezone }
    }
}

impl Filter for HumanDateFormat {
    fn filter(&self, in_date_time: &Value, _args: &HashMap<String, Value>) -> tera::Result<Value> {
        if let Ok(date) = from_value::<NaiveDate>(in_date_time.clone()) {
            Ok(Value::String(date.format("%A, %-d %B, %C%y").to_string()))
        } else if let Ok(date) = from_value::<DateTime<Utc>>(in_date_time.clone()) {
            Ok(Value::String(
                date.with_timezone(&self.timezone)
                    .format("%A, %-d %B, %C%y")
                    .to_string(),
            ))
        } else {
            Err(tera::Error::msg(format!(
                "Could not date format {}",
                in_date_time
            )))
        }
    }
    fn is_safe(&self) -> bool {
        true
    }
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
        .map(Value::String)
}

struct WeekDayFormat {
    timezone: chrono_tz::Tz,
}

impl WeekDayFormat {
    pub fn new(timezone: chrono_tz::Tz) -> Self {
        Self { timezone }
    }
}

impl Filter for WeekDayFormat {
    fn filter(&self, in_date: &Value, _args: &HashMap<String, Value>) -> tera::Result<Value> {
        if let Ok(date) = from_value::<NaiveDate>(in_date.clone()) {
            Ok(Value::String(format!(
                "{} {}",
                date.weekday(),
                Ordinal(date.day())
            )))
        } else if let Ok(date) = from_value::<DateTime<Utc>>(in_date.clone()) {
            let tz_date = date.with_timezone(&self.timezone);
            Ok(Value::String(format!(
                "{} {}",
                tz_date.weekday(),
                Ordinal(tz_date.day())
            )))
        } else {
            Err(tera::Error::msg(format!(
                "Cannot extract weekday from {}",
                in_date
            )))
        }
    }

    fn is_safe(&self) -> bool {
        true
    }
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
    let mut current_image: Option<Tag> = None;
    let mut image_text = String::new();
    let mut in_codeblock = false;
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
            Tag::Image {
                link_type: _,
                dest_url,
                title: _,
                id: _,
            } if dest_url.starts_with("!!") => {
                current_image = Some(tag.clone());
                Event::Text("".into())
            }
            Tag::CodeBlock(CodeBlockKind::Fenced(lang)) if !lang.is_empty() => {
                in_codeblock = true;
                Event::Html(
                    format!(
                        "<pre class=\"fenced-code language-{}\"><code class=\"language-{}\">",
                        lang, lang
                    )
                    .into(),
                )
            }
            _ => event,
        },
        Event::Code(code) if code.starts_with("$$") && code.ends_with("$$") => {
            let mathml = latex_to_mathml(code[2..(code.len() - 2)].into(), DisplayStyle::Inline)
                .unwrap_or("Bad Math!".into());
            Event::Html(mathml.into())
        }
        Event::Text(txt) if current_image.is_some() => {
            image_text.push_str(&format!(r#" alt="{}" "#, txt));
            Event::Text("".into())
        }
        Event::End(tag) => match tag {
            TagEnd::Image if current_image.is_some() => {
                if let Some(Tag::Image {
                    dest_url,
                    title,
                    id: _,
                    link_type: _,
                }) = current_image.clone()
                {
                    let dest_args: Vec<&str> = dest_url.split("?").collect();
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

                    html.push_str(&format!(r#"<img src="{}" title="{}""#, dest, title));

                    if let Some(a) = args.get("class") {
                        html.push_str(&format!(r#" class="{}" "#, a));
                    }

                    html.push_str("></picture>");
                    current_image = None;
                    image_text = String::new();
                    Event::Html(html.into())
                } else {
                    Event::Html("".into())
                }
            }
            TagEnd::CodeBlock if in_codeblock => {
                in_codeblock = false;
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
            parser
                .take_while(|e| !matches!(e, Event::Html(node) if node.starts_with("<blog-cut>"))),
        ),
        false => pulldown_cmark::html::push_html(&mut html_output, parser),
    };

    Ok(Value::String(html_output))
}
