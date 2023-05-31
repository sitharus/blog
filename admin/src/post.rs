use std::collections::HashMap;

use shared::{
    database,
    types::{Post, PostStatus},
    utils::{parse_into, post_body, render_html, render_redirect},
};

use crate::{
    common::{get_common, Common},
    types::AdminMenuPages,
};

use super::filters;
use super::response;
use super::session;
use anyhow::anyhow;
use askama::Template;
use cgi;
use chrono::{offset::Utc, NaiveDate};
use regex::Regex;
use serde::Deserialize;
use sqlx::{query, query_as};

#[derive(Template)]
#[template(path = "new_post.html")]
struct NewPost<'a> {
    common: Common,
    title: &'a str,
    body: &'a str,
    date: &'a NaiveDate,
    status: PostStatus,
}

#[derive(Template)]
#[template(path = "edit_post.html")]
struct EditPost<'a> {
    common: Common,
    title: &'a str,
    body: &'a str,
    date: &'a NaiveDate,
    status: PostStatus,
}

#[derive(Deserialize)]
struct NewPostRequest {
    title: String,
    body: String,
    date: NaiveDate,
    status: PostStatus,
}

#[derive(Template)]
#[template(path = "manage_posts.html")]
struct ManagePosts {
    common: Common,
    public_base_url: String,
    posts: Vec<Post>,
    current_page: i64,
    items_per_page: i64,
    post_count: i64,
    page_count: i64,
}

pub async fn new_post(request: &cgi::Request) -> anyhow::Result<cgi::Response> {
    let mut connection = database::connect_db().await?;
    let session::Session { user_id, .. } = session::session_id(&mut connection, &request).await?;

    let common = get_common(&mut connection, AdminMenuPages::NewPost).await?;

    if request.method() == "POST" {
        let req: NewPostRequest = post_body(request)?;

        let invalid_chars = Regex::new(r"[^a-z0-9_-]+")?;
        let mut initial_slug = req.title.clone();
        initial_slug.make_ascii_lowercase();
        let slug = invalid_chars
            .replace_all(&initial_slug, " ")
            .trim()
            .replace(" ", "_");
        let final_slug: &str = &slug.to_owned();
        let result = sqlx::query!(
            r#"
INSERT INTO posts(
    author_id, post_date, created_date, updated_date, state, url_slug, title, body
)
VALUES($1, $6, current_timestamp, current_timestamp, $5, $2, $3, $4)"#,
            user_id,
            final_slug,
            req.title,
            req.body,
            &req.status as &PostStatus,
            req.date,
        )
        .execute(&mut connection)
        .await?;
        return if result.rows_affected() == 1 {
            Ok(response::redirect_response("dashboard"))
        } else {
            let content = NewPost {
                common,
                title: req.title.as_str(),
                body: req.body.as_str(),
                status: req.status,
                date: &req.date,
            };
            render_html(content)
        };
    }

    let content = NewPost {
        common,
        title: "",
        body: "",
        status: PostStatus::Draft,
        date: &Utc::now().date_naive(),
    };
    render_html(content)
}

pub async fn edit_post(
    request: &cgi::Request,
    query: HashMap<String, String>,
) -> anyhow::Result<cgi::Response> {
    let mut connection = database::connect_db().await?;
    let id: i32 = query
        .get("id")
        .ok_or(anyhow!("Could not find id"))
        .and_then(parse_into)?;

    if request.method() == "POST" {
        let req: NewPostRequest = post_body(request)?;
        let status = req.status.clone();
        query!(
            "UPDATE posts SET title=$1, body=$2, state=$3, post_date = $4 WHERE id=$5",
            req.title,
            req.body,
            req.status as PostStatus,
            req.date,
            id
        )
        .execute(&mut connection)
        .await?;
        if status == PostStatus::Published {
            return render_redirect("posts");
        }
    }
    let post = sqlx::query!(
        r#"SELECT title, body, state as "state: PostStatus", post_date  FROM posts WHERE id = $1"#,
        id
    )
    .fetch_one(&mut connection)
    .await?;

    let common = get_common(&mut connection, AdminMenuPages::Posts).await?;

    let content = EditPost {
        common,
        title: &post.title,
        body: &post.body,
        status: post.state,
        date: &post.post_date,
    };
    render_html(content)
}

pub async fn manage_posts(query: HashMap<String, String>) -> anyhow::Result<cgi::Response> {
    let mut connection = database::connect_db().await?;

    let current_page = query.get("page").and_then(|p| p.parse().ok()).unwrap_or(0);
    let items_per_page = query.get("page").and_then(|p| p.parse().ok()).unwrap_or(20);

    let public_base_url =
        query!("SELECT value FROM blog_settings WHERE setting_name='comment_cgi_url'")
            .fetch_one(&mut connection)
            .await?;

    let posts = query_as!(
        Post,
        r#"SELECT id, author_id, post_date, created_date, updated_date, state as "state: PostStatus", url_slug, title, body FROM posts ORDER BY post_date DESC OFFSET $1 ROWS FETCH NEXT $2 ROWS ONLY"#,
        items_per_page * current_page,
        items_per_page
    )
    .fetch_all(&mut connection)
    .await?;

    let count = query!("SELECT COUNT(*) AS count FROM posts")
        .fetch_one(&mut connection)
        .await?;
    let post_count = count.count.unwrap_or_default();
    let page_count = (post_count as f64 / items_per_page as f64).ceil() as i64;

    let common = get_common(&mut connection, AdminMenuPages::Posts).await?;
    let content = ManagePosts {
        common,
        posts,
        current_page,
        items_per_page,
        post_count,
        page_count,
        public_base_url: public_base_url.value,
    };

    render_html(content)
}
