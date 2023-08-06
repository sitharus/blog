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
use super::types::PostRequest;
use anyhow::anyhow;
use askama::Template;
use cgi;
use chrono::{offset::Utc, NaiveDate};
use regex::Regex;
use sqlx::{query, query_as, PgConnection};

struct DisplayTag {
    id: i32,
    name: String,
}

#[derive(Template)]
#[template(path = "new_post.html")]
struct NewPost<'a> {
    common: Common,
    title: &'a str,
    body: &'a str,
    slug: &'a str,
    song: Option<&'a str>,
    mood: Option<&'a str>,
    summary: Option<&'a str>,
    date: &'a NaiveDate,
    status: PostStatus,
    tags: Vec<i32>,
    all_tags: Vec<DisplayTag>,
}

#[derive(Template)]
#[template(path = "edit_post.html")]
struct EditPost<'a> {
    common: Common,
    title: &'a str,
    body: &'a str,
    slug: &'a str,
    song: Option<&'a str>,
    mood: Option<&'a str>,
    summary: Option<&'a str>,
    date: &'a NaiveDate,
    status: PostStatus,
    tags: Vec<i32>,
    all_tags: Vec<DisplayTag>,
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

async fn get_tags(connection: &mut PgConnection) -> anyhow::Result<Vec<DisplayTag>> {
    Ok(
        query_as!(DisplayTag, "SELECT id, name FROM tags ORDER BY name")
            .fetch_all(connection)
            .await?,
    )
}

pub async fn new_post(request: &cgi::Request) -> anyhow::Result<cgi::Response> {
    let mut connection = database::connect_db().await?;
    let session::Session { user_id, .. } = session::session_id(&mut connection, &request).await?;

    let common = get_common(&mut connection, AdminMenuPages::NewPost).await?;

    if request.method() == "POST" {
        let req: PostRequest = post_body(request)?;

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
            r#"
INSERT INTO posts(
    author_id, post_date, created_date, updated_date, state,
    url_slug, title, body, song, mood, summary
)
VALUES($1, $6, current_timestamp, current_timestamp, $5, $2, $3, $4, $7, $8, $9)
RETURNING id"#,
            user_id,
            final_slug,
            req.title,
            req.body,
            &req.status as &PostStatus,
            req.date,
            req.song,
            req.mood,
            req.summary,
        )
        .fetch_optional(&mut connection)
        .await?;
        return if let Some(row) = result {
            if let Some(tags) = req.tags {
                for i in tags {
                    query!(
                        "INSERT INTO post_tag(post_id, tag_id) VALUES($1, $2)",
                        row.id,
                        i
                    )
                    .execute(&mut connection)
                    .await?;
                }
            }
            Ok(response::redirect_response("dashboard"))
        } else {
            let content = NewPost {
                common,
                title: req.title.as_str(),
                body: req.body.as_str(),
                status: req.status,
                date: &req.date,
                slug: final_slug,
                mood: req.mood.as_deref(),
                song: req.song.as_deref(),
                summary: req.song.as_deref(),
                tags: req.tags.unwrap_or(vec![]),
                all_tags: get_tags(&mut connection).await?,
            };
            render_html(content)
        };
    }

    let content = NewPost {
        common,
        title: "",
        body: "",
        slug: "",
        mood: None,
        song: None,
        summary: None,
        status: PostStatus::Draft,
        date: &Utc::now().date_naive(),
        tags: vec![],
        all_tags: get_tags(&mut connection).await?,
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
        let req: PostRequest = post_body(request)?;
        let status = req.status.clone();
        query!(
            "UPDATE posts SET title=$1, body=$2, state=$3, post_date = $4, url_slug=$5, song=$6, mood=$7, summary=$8 WHERE id=$9",
            req.title,
            req.body,
            req.status as PostStatus,
            req.date,
            req.slug,
            req.song,
            req.mood,
            req.summary,
            id
        )
        .execute(&mut connection)
        .await?;

        query!("DELETE FROM post_tag WHERE post_id=$1", id)
            .execute(&mut connection)
            .await?;
        if let Some(tags) = req.tags {
            for i in tags {
                query!(
                    "INSERT INTO post_tag(post_id, tag_id) VALUES($1, $2)",
                    id,
                    i
                )
                .execute(&mut connection)
                .await?;
            }
        }
        if status == PostStatus::Published {
            return render_redirect("posts");
        }
    }
    let post = sqlx::query!(
        r#"SELECT title, body, url_slug, state as "state: PostStatus", post_date, song, mood, summary, array_agg(tag_id) AS tags FROM posts LEFT JOIN post_tag ON post_tag.post_id = posts.id WHERE id = $1 GROUP BY posts.id"#,
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
        slug: &post.url_slug,
        mood: post.mood.as_deref(),
        song: post.song.as_deref(),
        summary: post.summary.as_deref(),
        tags: post.tags.unwrap_or(vec![]),
        all_tags: get_tags(&mut connection).await?,
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
