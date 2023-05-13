use std::collections::HashMap;

use crate::types::AdminMenuPages;
use crate::types::Post;
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
use serde::Deserialize;
use sqlx::{query, query_as};
use regex::Regex;

#[derive(Template)]
#[template(path = "new_post.html")]
struct NewPost<'a> {
    title: &'a str,
    body: &'a str,
    selected_menu_item: AdminMenuPages,
}

#[derive(Template)]
#[template(path = "edit_post.html")]
struct EditPost<'a> {
    title: &'a str,
    body: &'a str,
    selected_menu_item: AdminMenuPages,
}

#[derive(Deserialize)]
struct NewPostRequest {
    title: String,
    body: String,
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
		let slug = invalid_chars.replace_all(&initial_slug, " ");
		let final_slug: &str = &slug.to_owned();
        let result = sqlx::query!("INSERT INTO posts(author_id, post_date, updated_date, url_slug, title, body) VALUES($1, current_timestamp, current_timestamp, $2, $3, $4)",
					 user_id,
					 final_slug.trim(),
					 req.title,
					 req.body
		).execute(&mut connection).await?;
        return if result.rows_affected() == 1 {
            Ok(response::redirect_response("dashboard"))
        } else {
            let content = NewPost {
                title: req.title.as_str(),
                body: req.body.as_str(),
                selected_menu_item: AdminMenuPages::NewPost,
            };
            Ok(cgi::html_response(200, content.render().unwrap()))
        };
    }

    let content = NewPost {
        title: "",
        body: "",
        selected_menu_item: AdminMenuPages::NewPost,
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
            "UPDATE posts SET title=$1, body=$2 WHERE id=$3",
            req.title,
            req.body,
            id
        )
        .execute(&mut connection)
        .await?;
		render_redirect("posts")
    } else {
		let post = sqlx::query!("SELECT title, body FROM posts WHERE id = $1", id)
			.fetch_one(&mut connection)
			.await?;

		let content = EditPost {
			title: &post.title,
			body: &post.body,
			selected_menu_item: AdminMenuPages::Posts,
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
        "SELECT * FROM posts ORDER BY post_date DESC OFFSET $1 ROWS FETCH NEXT $2 ROWS ONLY",
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
