use std::collections::HashMap;

use anyhow::{anyhow, bail};
use cgi::{binary_response, html_response, text_response};
use chrono::Datelike;
use serde_json::{from_value, Value};
use shared::generator::index::index_content;
use shared::generator::static_content::{get_static_content, StaticContent};
use shared::generator::{
    get_common, pages::generate_single_page, templates::load_templates, types::Generator,
};
use shared::types::{CommonData, HydratedPost};
use shared::utils::parse_into;
use tera::Function;

use crate::generator::{get_content, PageContent};
use crate::types::PageGlobals;

pub async fn prepublished(
    _request: &cgi::Request,
    globals: PageGlobals,
) -> anyhow::Result<cgi::Response> {
    let kind = globals.query.get("kind").ok_or(anyhow!("No target"))?;
    match kind.as_str() {
        "post" => page(globals).await,
        "index" => index(globals).await,
        "static" => staticpath(globals).await,
        _ => bail!("Not implemented"),
    }
}

async fn get_generator<'a>(
    globals: &'a PageGlobals,
    common: &'a CommonData,
) -> anyhow::Result<Generator<'a>> {
    let mut tera = load_templates(&globals.connection_pool, globals.site_id, common).await?;

    let site_url = SiteUrlBuilder {};
    tera.register_filter("posturl", page_url);
    tera.register_function("buildurl", site_url);

    Ok(Generator {
        output_path: "",
        pool: &globals.connection_pool,
        common,
        tera,
        site_id: globals.site_id,
    })
}

async fn page(globals: PageGlobals) -> anyhow::Result<cgi::Response> {
    let page_id: i32 = globals
        .query
        .get("id")
        .ok_or(anyhow!("No ID"))
        .and_then(|s| parse_into(&s))?;
    let common = get_common(&globals.connection_pool, globals.site_id).await?;
    let gen = get_generator(&globals, &common).await?;
    let result = generate_single_page(page_id, &gen).await?;

    Ok(html_response(200, result))
}

async fn index(globals: PageGlobals) -> anyhow::Result<cgi::Response> {
    let PageContent { posts, common } = get_content(&globals).await?;
    let total_pages = 1;
    let page_number = 1;
    let chunk = posts
        .chunks(10)
        .next()
        .ok_or(anyhow::anyhow!("No post!"))?
        .iter()
        .collect();
    let gen = get_generator(&globals, &common).await?;

    let result = index_content(chunk, &gen, page_number, total_pages)?;
    Ok(html_response(200, result))
}

async fn staticpath(globals: PageGlobals) -> anyhow::Result<cgi::Response> {
    if let Some(content) = globals.query.get("static") {
        let common = get_common(&globals.connection_pool, globals.site_id).await?;
        let gen = get_generator(&globals, &common).await?;
        let StaticContent {
            content,
            content_type,
        } = get_static_content(&gen, content).await?;
        Ok(binary_response(
            200,
            Some(content_type.as_str()),
            content.as_bytes().to_vec(),
        ))
    } else {
        Ok(text_response(404, "Not Found"))
    }
}

fn page_url(post: &Value, args: &HashMap<String, Value>) -> tera::Result<Value> {
    match post {
        Value::Object(_) => {
            let target: Option<String> = args
                .get("target")
                .and_then(|t| from_value::<String>(t.clone()).ok());
            let post: HydratedPost =
                serde_json::from_value(post.clone()).map_err(tera::Error::from)?;

            let url = match target.as_deref() {
                Some("year") => format!(
                    "?action=prepublished&kind=year_index&year={}",
                    post.post_date.year()
                ),
                Some("month") => format!(
                    "?action=prepublished&kind=month_index&year={}&month={}",
                    post.post_date.year(),
                    post.post_date.format("%B")
                ),
                _ => format!("?action=prepublished&kind=post&id={}", post.id),
            };
            Ok(Value::String(url))
        }
        Value::String(s) => Ok(Value::String(format!("?action=prepublished&path={}", s))),
        _ => Err(tera::Error::msg(format!(
            "Not able to build a URL from {:?}",
            post
        ))),
    }
}

struct SiteUrlBuilder {}

impl Function for SiteUrlBuilder {
    fn call(&self, args: &HashMap<String, Value>) -> tera::Result<Value> {
        if let Some(static_url) = args.get("static") {
            match static_url {
                Value::String(value) => Ok(Value::String(format!(
                    "?action=prepublished&kind=static&static={}",
                    value
                ))),
                _ => Err(tera::Error::msg("Static url must be a string")),
            }
        } else if args.get("home").is_some() {
            Ok(Value::String("?action=prepublished&kind=index".into()))
        } else if let Some(page) = args.get("page") {
            match page {
                Value::Object(page_obj) => match page_obj.get("url_slug") {
                    Some(slug) => Ok(Value::String(format!(
                        "?action=prepublished&kind=page&slug={}",
                        slug
                    ))),
                    _ => Err(tera::Error::msg("Not a page")),
                },
                _ => Err(tera::Error::msg("Not a page")),
            }
        } else {
            Err(tera::Error::msg("Unsupported url builder"))
        }
    }
    fn is_safe(&self) -> bool {
        true
    }
}
