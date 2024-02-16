use anyhow::anyhow;
use askama::Template;
use chrono::{DateTime, Utc};
use regex::Regex;
use serde::Deserialize;
use shared::utils::{parse_into, post_body, render_html};
use sqlx::{query, query_as};

use crate::{
    common::{get_common, Common},
    filters, response,
    types::{AdminMenuPages, PageGlobals},
};

#[derive(Deserialize)]
struct NewPageRequest {
    title: String,
    body: String,
    slug: String,
}

#[derive(Template)]
#[template(path = "manage_pages.html")]
struct ManagePages {
    common: Common,
    pages: Vec<PageListItem>,
}

#[derive(Template)]
#[template(path = "new_page.html")]
struct NewPage {
    common: Common,
    title: String,
    slug: String,
    body: String,
}

#[derive(Template)]
#[template(path = "edit_page.html")]
struct EditPage {
    common: Common,
    title: String,
    body: String,
    slug: String,
}

struct PageListItem {
    id: i32,
    date_updated: DateTime<Utc>,
    url_slug: String,
    title: String,
}

pub async fn manage_pages(globals: PageGlobals) -> anyhow::Result<cgi::Response> {
    let pages = query_as!(
        PageListItem,
        "SELECT id, title, date_updated, url_slug FROM pages WHERE site_id=$1 ORDER BY title",
        globals.site_id
    )
    .fetch_all(&globals.connection_pool)
    .await?;
    let common = get_common(&globals, AdminMenuPages::Pages).await?;

    render_html(ManagePages { common, pages })
}

pub async fn new_page(
    request: &cgi::Request,
    globals: PageGlobals,
) -> anyhow::Result<cgi::Response> {
    let common = get_common(&globals, AdminMenuPages::Pages).await?;

    if request.method() == "POST" {
        let req: NewPageRequest = post_body(&request)?;
        let user_id = globals.session.user_id;
        let invalid_chars = Regex::new(r"[^a-z0-9_-]+")?;
        let mut initial_slug = if req.slug == "" {
            req.title.clone()
        } else {
            req.slug.clone()
        };
        initial_slug.make_ascii_lowercase();
        let slug = invalid_chars
            .replace_all(&initial_slug, " ")
            .trim()
            .replace(" ", "_");
        let final_slug: &str = &slug.to_owned();

        let result = sqlx::query!(
            "
INSERT INTO pages (author_id, date_updated, url_slug, title, body, site_id)
VALUES ($1, current_timestamp,  $2, $3, $4, $5)
",
            user_id,
            final_slug,
            req.title,
            req.body,
            globals.site_id
        )
        .execute(&globals.connection_pool)
        .await;

        match result {
            Ok(x) if x.rows_affected() == 1 => Ok(response::redirect_response("manage_pages")),
            _ => render_html(NewPage {
                common,
                title: req.title,
                slug: final_slug.into(),
                body: req.body,
            }),
        }
    } else {
        render_html(NewPage {
            common,
            title: "".into(),
            slug: "".into(),
            body: "".into(),
        })
    }
}

pub async fn edit_post(
    request: &cgi::Request,
    globals: PageGlobals,
) -> anyhow::Result<cgi::Response> {
    let id: i32 = globals
        .query
        .get("id")
        .ok_or(anyhow!("Could not find id"))
        .and_then(parse_into)?;

    let common = get_common(&globals, AdminMenuPages::Pages).await?;

    if request.method() == "POST" {
        let req: NewPageRequest = post_body(&request)?;
        let response = query!(
            "UPDATE pages SET title=$1, url_slug=$2, body=$3 WHERE id=$4 AND site_id=$5",
            req.title,
            req.slug,
            req.body,
            id,
            globals.site_id
        )
        .execute(&globals.connection_pool)
        .await;

        match response {
            Ok(x) if x.rows_affected() == 1 => Ok(response::redirect_response("manage_pages")),
            _ => render_html(EditPage {
                common,
                title: req.title,
                slug: req.slug,
                body: req.body,
            }),
        }
    } else {
        let page = query!(
            "SELECT title, url_slug, body FROM pages WHERE id=$1 AND site_id=$2",
            id,
            globals.site_id
        )
        .fetch_one(&globals.connection_pool)
        .await?;
        render_html(EditPage {
            common,
            title: page.title.into(),
            slug: page.url_slug.into(),
            body: page.body.into(),
        })
    }
}
