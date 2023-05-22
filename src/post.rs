use std::collections::HashMap;

use crate::types::AdminMenuPages;
use crate::types::{Post, PostStatus};
use crate::utils::parse_into;
use crate::utils::post_body;
use crate::utils::render_html;
use crate::utils::render_redirect;

use super::database;
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
    title: &'a str,
    body: &'a str,
    date: &'a NaiveDate,
    selected_menu_item: AdminMenuPages,
    status: PostStatus,
}

#[derive(Template)]
#[template(path = "edit_post.html")]
struct EditPost<'a> {
    title: &'a str,
    body: &'a str,
    date: &'a NaiveDate,
    selected_menu_item: AdminMenuPages,
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
    selected_menu_item: AdminMenuPages,
    posts: Vec<Post>,
    current_page: i64,
    items_per_page: i64,
    post_count: i64,
    page_count: i64,
}

pub async fn new_post(request: &cgi::Request) -> anyhow::Result<cgi::Response> {
    let mut connection = database::connect_db().await?;
    let session::Session { user_id, .. } = session::session_id(&mut connection, &request).await?;

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
                title: req.title.as_str(),
                body: req.body.as_str(),
                selected_menu_item: AdminMenuPages::NewPost,
                status: req.status,
                date: &req.date,
            };
            render_html(content)
        };
    }

    let content = NewPost {
        title: "",
        body: "",
        selected_menu_item: AdminMenuPages::NewPost,
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
        render_redirect("posts")
    } else {
        let post = sqlx::query!(
            r#"SELECT title, body, state as "state: PostStatus", post_date  FROM posts WHERE id = $1"#,
            id
        )
        .fetch_one(&mut connection)
        .await?;

        let content = EditPost {
            title: &post.title,
            body: &post.body,
            selected_menu_item: AdminMenuPages::Posts,
            status: post.state,
            date: &post.post_date,
        };
        render_html(content)
    }
}

pub async fn manage_posts(query: HashMap<String, String>) -> anyhow::Result<cgi::Response> {
    let mut connection = database::connect_db().await?;

    let current_page = query.get("page").and_then(|p| p.parse().ok()).unwrap_or(0);
    let items_per_page = query.get("page").and_then(|p| p.parse().ok()).unwrap_or(20);

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

    let content = ManagePosts {
        selected_menu_item: AdminMenuPages::Posts,
        posts,
        current_page,
        items_per_page,
        post_count,
        page_count,
    };

    render_html(content)
}
