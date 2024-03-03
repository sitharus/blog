use crate::database::connect_db;
use crate::types::{CommonData, HydratedPost, ImageMetadata, Link, Media, PageLink};
use anyhow::anyhow;
use cgi::text_response;
use chrono::{Datelike, Utc};
use sqlx::PgPool;
use sqlx::{query, query_as, types::Json};
use std::collections::HashMap;
pub mod activitypub;
pub mod feeds;
pub mod filters;
pub mod index;
pub mod month_index;
pub mod pages;
pub mod posts;
pub mod templates;
pub mod types;
pub mod year_index;

pub async fn external_preview(id: i32) -> anyhow::Result<cgi::Response> {
    let connection = connect_db().await?;
    let maybe_post = query_as!(
        HydratedPost,
        r#"
SELECT
    posts.id as id,
    post_date,
    url_slug,
    title,
    body,
    song,
    mood,
    summary,
	site_id,
    users.display_name AS author_name,
    (SELECT COUNT(*) FROM comments WHERE comments.post_id = posts.id) AS comment_count,
    (SELECT array_agg(t.name) FROM tags t INNER JOIN post_tag pt ON pt.tag_id = t.id WHERE pt.post_id = posts.id) AS tags
FROM posts
INNER JOIN users
ON users.id = posts.author_id
WHERE state = 'preview'
AND posts.id=$1
"#,
        id
    )
    .fetch_optional(&connection)
    .await?;

    match maybe_post {
        Some(_post) => {
            Ok(text_response(200, "Currently broken"))

            /*
            let common = get_common(&connection, post.site_id).await?;
            let post_page = PostPage {
                title: &post.title,
                post: &post,
                common: &common,
                comments: [].into(),
            };

            render_html(post_page)*/
        }
        _ => Ok(text_response(404, "404 Not Found")),
    }
}

pub async fn get_common(connection: &PgPool, site_id: i32) -> anyhow::Result<CommonData> {
    // TODO: Figure out how to use a &mut connection argument.
    let settings: HashMap<String, String>;
    let raw_settings = query!(
        "SELECT setting_name, value FROM blog_settings WHERE site_id=$1",
        site_id
    )
    .fetch_all(connection)
    .await?;
    settings = HashMap::from_iter(raw_settings.into_iter().map(|r| (r.setting_name, r.value)));

    let links = query_as!(
        Link,
        "SELECT title, destination FROM external_links WHERE site_id=$1 ORDER BY position",
        site_id
    )
    .fetch_all(connection)
    .await?;

    let page_links = query_as!(
        PageLink,
        "SELECT title, url_slug FROM pages WHERE site_id=$1 ORDER BY title",
        site_id
    )
    .fetch_all(connection)
    .await?;

    let earliest_post = query!(
        "SELECT post_date FROM posts WHERE site_id=$1 ORDER BY post_date ASC LIMIT 1",
        site_id
    )
    .fetch_one(connection)
    .await?;
    let earliest_year = earliest_post.post_date.year();
    let current_year = Utc::now().year();
    let mut years: Vec<i32> = (earliest_year..=current_year).collect();
    years.reverse();

    let media_rows =
        query!(r#"SELECT id, file, metadata AS "metadata: Json<ImageMetadata>"  FROM media WHERE site_id=$1"#, site_id)
            .fetch_all(connection)
            .await?;

    let media = HashMap::from_iter(media_rows.into_iter().map(|m| -> (i32, Media) {
        (
            m.id,
            Media {
                id: m.id,
                file: m.file,
                metadata: m.metadata.unwrap().as_ref().to_owned(),
            },
        )
    }));

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
        media_base_url: settings
            .get("media_base_url")
            .ok_or(anyhow!("No media url set"))?
            .to_owned(),
        archive_years: years,
        timezone: settings
            .get("timezone")
            .and_then(|x| x.parse().ok())
            .unwrap_or(chrono_tz::UTC)
            .to_owned(),
        links,
        page_links,
        media,
    })
}
