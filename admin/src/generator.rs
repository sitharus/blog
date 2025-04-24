use crate::response::redirect_response;
use crate::types::{PageGlobals, PostRequest};
use shared::generator::feeds::{generate_atom_feed, generate_rss_feed};
use shared::generator::get_common;
use shared::generator::index::{generate_index_pages, generate_tag_indexes};
use shared::generator::month_index::generate_month_index_pages;
use shared::generator::pages::generate_pages;
use shared::generator::posts::{generate_post_html, generate_post_page};
use shared::generator::static_content::generate_static;
use shared::generator::templates::load_templates;
use shared::generator::types::Generator;
use shared::generator::year_index::generate_year_index_pages;
use shared::types::{CommonData, HydratedPost};
use shared::utils::post_body;

use tokio::fs::{create_dir, try_exists};

use sqlx::{query, query_as};
use std::env;

pub async fn preview_page(
    request: &cgi::Request,
    globals: PageGlobals,
) -> anyhow::Result<cgi::Response> {
    let common = get_common(&globals.connection_pool, globals.site_id).await?;
    let user = query!(
        "SELECT display_name FROM users WHERE id=$1",
        globals.session.user_id
    )
    .fetch_one(&globals.connection_pool)
    .await?;

    let data: PostRequest = post_body(request)?;

    let mut tags: Vec<String> = Vec::new();
    if let Some(tag_ids) = data.tags {
        for t in tag_ids {
            let result = query!(
                "SELECT name FROM tags WHERE id=$1 AND site_id=$2",
                t,
                globals.site_id
            )
            .fetch_one(&globals.connection_pool)
            .await?;
            tags.push(result.name);
        }
    }

    let post_date = data
        .date
        .and_local_timezone(common.timezone)
        .earliest()
        .ok_or(anyhow::anyhow!("Could not set timezone on post time"))?
        .to_utc();

    let post = HydratedPost {
        id: 0,
        post_date,
        url_slug: "preview".into(),
        title: data.title,
        body: data.body,
        song: data.song,
        mood: data.mood,
        summary: data.summary,
        author_name: user.display_name,
        comment_count: Some(0),
        tags: Some(tags),
        site_id: globals.site_id,
    };

    let tera = load_templates(&globals.connection_pool, globals.site_id, &common).await?;

    let gen = Generator {
        output_path: "",
        pool: &globals.connection_pool,
        common: &common,
        tera,
        site_id: globals.site_id,
    };
    let post_page = generate_post_html(&gen, &post).await?;

    Ok(cgi::html_response(200, post_page))
}

pub struct PageContent {
    pub posts: Vec<HydratedPost>,
    pub common: CommonData,
}

pub async fn get_content(globals: &PageGlobals) -> anyhow::Result<PageContent> {
    let posts = query_as!(
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
    users.display_name AS author_name,
    (SELECT COUNT(*) FROM comments WHERE comments.post_id = posts.id) AS comment_count,
    (SELECT array_agg(t.name) FROM tags t INNER JOIN post_tag pt ON pt.tag_id = t.id WHERE pt.post_id = posts.id) AS tags,
	site_id
FROM posts
INNER JOIN users
ON users.id = posts.author_id
WHERE state = 'published'
AND posts.site_id = $1
AND posts.post_date <= CURRENT_TIMESTAMP
ORDER BY post_date DESC
"#, globals.site_id
    )
    .fetch_all(&globals.connection_pool)
    .await?;

    let common = get_common(&globals.connection_pool, globals.site_id).await?;

    Ok(PageContent { posts, common })
}

pub async fn regenerate_blog(globals: &PageGlobals) -> anyhow::Result<cgi::Response> {
    let output_path_base =
        env::var("BLOG_OUTPUT_PATH").expect("Environment variable BLOG_OUTPUT_PATH is required");
    let output_path = format!("{}/{}", output_path_base, globals.site_id);
    let static_output_path = format!("{}/{}", output_path, "static");

    if !(try_exists(&static_output_path).await?) {
        create_dir(&static_output_path).await?;
    }

    let PageContent { posts, common } = get_content(globals).await?;
    if posts.is_empty() {
        return Ok(redirect_response("dashboard", globals.site_id));
    }

    let tera = load_templates(&globals.connection_pool, globals.site_id, &common).await?;
    let gen = Generator {
        output_path: &output_path,
        pool: &globals.connection_pool,
        common: &common,
        tera,
        site_id: globals.site_id,
    };

    for post in &posts {
        generate_post_page(&gen, post).await?;
    }

    generate_index_pages(
        posts.iter().collect::<Vec<&HydratedPost>>().into_iter(),
        &gen,
    )
    .await?;

    generate_month_index_pages(&posts, &gen).await?;
    generate_year_index_pages(&posts, &gen).await?;
    generate_rss_feed(&posts, &gen).await?;
    generate_atom_feed(&posts, &gen).await?;
    generate_tag_indexes(&posts, &gen).await?;
    generate_pages(&gen).await?;
    generate_static(&gen, &static_output_path).await?;

    Ok(redirect_response("dashboard", globals.site_id))
}
