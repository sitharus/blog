use shared::{
    settings::get_settings_struct,
    types::{Post, PostStatus},
    utils::{parse_into, post_body, render_html, render_redirect},
};

use crate::{
    common::{get_common, Common},
    types::{AdminMenuPages, PageGlobals},
};

use super::filters;
use super::response;
use super::types::PostRequest;
use anyhow::anyhow;
use askama::Template;
use chrono::{offset::Utc, DateTime};
use regex::Regex;
use sqlx::{query, query_as, PgPool};

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
    date: &'a DateTime<Utc>,
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
    date: &'a DateTime<Utc>,
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

async fn get_tags(connection: &PgPool) -> anyhow::Result<Vec<DisplayTag>> {
    Ok(
        query_as!(DisplayTag, "SELECT id, name FROM tags ORDER BY name")
            .fetch_all(connection)
            .await?,
    )
}

pub async fn new_post(
    request: &cgi::Request,
    globals: PageGlobals,
) -> anyhow::Result<cgi::Response> {
    let common = get_common(&globals, AdminMenuPages::NewPost).await?;

    if request.method() == "POST" {
        let req: PostRequest = post_body(request)?;

        let invalid_chars = Regex::new(r"[^a-z0-9_-]+")?;
        let mut initial_slug = if req.slug.is_empty() {
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
    url_slug, title, body, song, mood, summary, site_id
)
VALUES($1, $6, current_timestamp, current_timestamp, $5, $2, $3, $4, $7, $8, $9, $10)
RETURNING id"#,
            globals.session.user_id,
            final_slug,
            req.title,
            req.body,
            &req.status as &PostStatus,
            req.date,
            req.song,
            req.mood,
            req.summary,
            globals.site_id,
        )
        .fetch_optional(&globals.connection_pool)
        .await?;
        return if let Some(row) = result {
            if let Some(tags) = req.tags {
                for i in tags {
                    query!(
                        "INSERT INTO post_tag(post_id, tag_id) VALUES($1, $2)",
                        row.id,
                        i
                    )
                    .execute(&globals.connection_pool)
                    .await?;
                }
            }
            Ok(response::redirect_response("dashboard", globals.site_id))
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
                tags: req.tags.unwrap_or_default(),
                all_tags: get_tags(&globals.connection_pool).await?,
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
        date: &Utc::now(),
        tags: vec![],
        all_tags: get_tags(&globals.connection_pool).await?,
    };
    render_html(content)
}

pub async fn edit_post(
    request: &cgi::Request,
    globals: PageGlobals,
) -> anyhow::Result<cgi::Response> {
    let id: i32 = globals
        .query
        .get("id")
        .ok_or(anyhow!("Could not find id"))
        .and_then(|s| parse_into(s))?;

    if request.method() == "POST" {
        let req: PostRequest = post_body(request)?;
        let status = req.status.clone();
        query!(
            "UPDATE posts SET title=$1, body=$2, state=$3, post_date = $4, url_slug=$5, song=$6, mood=$7, summary=$8 WHERE id=$9 AND site_id=$10",
            req.title,
            req.body,
            req.status as PostStatus,
            req.date,
            req.slug,
            req.song,
            req.mood,
            req.summary,
            id,
            globals.site_id
        )
        .execute(&globals.connection_pool)
        .await?;

        query!("DELETE FROM post_tag WHERE post_id=$1", id)
            .execute(&globals.connection_pool)
            .await?;
        if let Some(tags) = req.tags {
            for i in tags {
                query!(
                    "INSERT INTO post_tag(post_id, tag_id) VALUES($1, $2)",
                    id,
                    i
                )
                .execute(&globals.connection_pool)
                .await?;
            }
        }
        if status == PostStatus::Published {
            return render_redirect("manage_posts", globals.site_id);
        }
    }
    let post = sqlx::query!(
        r#"
SELECT
    title, body, url_slug, state as "state: PostStatus", post_date, song, mood, summary,
    array_agg(tag_id) FILTER (WHERE tag_id IS NOT NULL) AS "tags?"
FROM posts
LEFT JOIN post_tag ON post_tag.post_id = posts.id
WHERE id = $1 AND site_id=$2
GROUP BY posts.id"#,
        id,
        globals.site_id
    )
    .fetch_one(&globals.connection_pool)
    .await?;

    let common = get_common(&globals, AdminMenuPages::Posts).await?;

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
        all_tags: get_tags(&globals.connection_pool).await?,
    };
    render_html(content)
}

pub async fn manage_posts(globals: PageGlobals) -> anyhow::Result<cgi::Response> {
    let current_page = globals
        .query
        .get("page")
        .and_then(|p| p.parse().ok())
        .unwrap_or(0);
    let items_per_page = globals
        .query
        .get("page")
        .and_then(|p| p.parse().ok())
        .unwrap_or(20);
    let settings = get_settings_struct(&globals.connection_pool, globals.site_id).await?;

    let public_base_url = settings.comment_cgi_url.clone();

    let posts = query_as!(
        Post,
        r#"SELECT id, author_id, post_date, created_date, updated_date, state as "state: PostStatus", url_slug, title, body FROM posts WHERE site_id=$3 ORDER BY post_date DESC OFFSET $1 ROWS FETCH NEXT $2 ROWS ONLY"#,
        items_per_page * current_page,
        items_per_page,
		globals.site_id
    )
    .fetch_all(&globals.connection_pool)
    .await?;

    let count = query!(
        "SELECT COUNT(*) AS count FROM posts WHERE site_id = $1",
        globals.site_id
    )
    .fetch_one(&globals.connection_pool)
    .await?;
    let post_count = count.count.unwrap_or_default();
    let page_count = (post_count as f64 / items_per_page as f64).ceil() as i64;

    let common = get_common(&globals, AdminMenuPages::Posts).await?;
    let content = ManagePosts {
        common,
        posts,
        current_page,
        items_per_page,
        post_count,
        page_count,
        public_base_url,
    };

    render_html(content)
}
