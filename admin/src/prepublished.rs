use anyhow::{anyhow, bail};
use cgi::html_response;
use shared::generator::{
    get_common, pages::generate_single_page, templates::load_templates, types::Generator,
};
use shared::types::HydratedPost;
use shared::utils::parse_into;
use sqlx::query_as;

use crate::types::PageGlobals;

pub async fn prepublished(
    request: &cgi::Request,
    globals: PageGlobals,
) -> anyhow::Result<cgi::Response> {
    let kind = globals.query.get("kind").ok_or(anyhow!("No target"))?;
    match kind.as_str() {
        "post" => page(globals).await,
        "index" => index(globals).await,
        _ => bail!("Not implemented"),
    }
}

async fn page(globals: PageGlobals) -> anyhow::Result<cgi::Response> {
    let page_id: i32 = globals
        .query
        .get("id")
        .ok_or(anyhow!("No ID"))
        .and_then(parse_into)?;
    let common = get_common(&globals.connection_pool, globals.site_id).await?;
    let tera = load_templates(&common)?;

    let gen = Generator {
        output_path: "",
        pool: &globals.connection_pool,
        common: &common,
        tera: &tera,
        site_id: globals.site_id,
    };

    let result = generate_single_page(page_id, &gen).await?;

    Ok(html_response(200, result))
}

async fn index(globals: PageGlobals) -> anyhow::Result<cgi::Response> {
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
ORDER BY post_date DESC
"#, globals.site_id
    )
    .fetch_all(&globals.connection_pool)
    .await?;

    bail!("Not implemented")
}
