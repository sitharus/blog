use crate::database::connect_db;
use crate::types::{CommonData, HydratedComment, HydratedPost, Link, PageLink};
use crate::utils::render_html;
use anyhow::anyhow;
use askama::Template;
use cgi::text_response;
use chrono::{Datelike, Utc};
use sqlx::{postgres::PgConnection, query, query_as};
use std::collections::HashMap;
pub mod filters;

#[derive(Template)]
#[template(path = "generated/post.html")]
struct PostPage<'a> {
    title: &'a String,
    post: &'a HydratedPost,
    common: &'a CommonData,
    comments: Vec<HydratedComment>,
}

pub async fn external_preview(id: i32) -> anyhow::Result<cgi::Response> {
    let mut connection = connect_db().await?;
    let maybe_post = query_as!(
        HydratedPost,
        "
SELECT posts.id as id, post_date, url_slug, title, body, users.display_name AS author_name, (SELECT COUNT(*) FROM comments WHERE comments.post_id = posts.id) AS comment_count
FROM posts
INNER JOIN users
ON users.id = posts.author_id
WHERE state = 'preview'
AND posts.id=$1
ORDER BY post_date DESC", id
    )
    .fetch_optional(&mut connection)
    .await?;

    match maybe_post {
        Some(post) => {
            let common = get_common(&mut connection).await?;
            let post_page = PostPage {
                title: &post.title,
                post: &post,
                common: &common,
                comments: [].into(),
            };

            render_html(post_page)
        }
        _ => Ok(text_response(404, "404 Not Found")),
    }
}

pub async fn get_common(connection: &mut PgConnection) -> anyhow::Result<CommonData> {
    // TODO: Figure out how to use a &mut connection argument.
    let settings: HashMap<String, String>;
    let raw_settings = query!("SELECT setting_name, value FROM blog_settings")
        .fetch_all(&mut *connection)
        .await?;
    settings = HashMap::from_iter(raw_settings.into_iter().map(|r| (r.setting_name, r.value)));

    let links = query_as!(
        Link,
        "SELECT title, destination FROM external_links ORDER BY position"
    )
    .fetch_all(&mut *connection)
    .await?;

    let page_links = query_as!(PageLink, "SELECT title, url_slug FROM pages ORDER BY title")
        .fetch_all(&mut *connection)
        .await?;

    let earliest_post = query!("SELECT post_date FROM posts ORDER BY post_date ASC LIMIT 1")
        .fetch_one(&mut *connection)
        .await?;
    let earliest_year = earliest_post.post_date.year();
    let current_year = Utc::now().year();
    let mut years: Vec<i32> = (earliest_year..=current_year).collect();
    years.reverse();

    Ok(CommonData {
        base_url: settings
            .get("base_url")
            .ok_or(anyhow!("No blog URL set"))?
            .to_owned(),
        blog_name: settings
            .get("blog_name")
            .ok_or(anyhow!("No blog name set"))?
            .to_owned(),
        static_base_url: settings
            .get("static_base_url")
            .ok_or(anyhow!("No base URL set"))?
            .to_owned(),
        comment_cgi_url: settings
            .get("comment_cgi_url")
            .ok_or(anyhow!("No CGI url set"))?
            .to_owned(),
        archive_years: years,
        links,
        page_links,
    })
}
