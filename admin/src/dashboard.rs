use crate::{
    common::{get_common, Common},
    types::AdminMenuPages,
};
use askama::Template;
use cgi;
use shared::{
    database,
    types::{Post, PostStatus},
    utils::render_html,
};
use sqlx::query_as;

use super::session;
use crate::filters;

#[derive(Template)]
#[template(path = "dashboard.html")]
struct Dashboard {
    common: Common,
    recent_posts: Vec<Post>,
}

pub async fn render(request: &cgi::Request) -> anyhow::Result<cgi::Response> {
    let mut connection = database::connect_db().await?;
    session::session_id(&mut connection, &request).await?;

    let recent_posts = query_as!(
        Post,
        r#"
SELECT id, author_id, post_date, created_date, updated_date, state as "state: PostStatus",
       url_slug, title, body
FROM posts
WHERE state = 'published'
ORDER BY post_date DESC
FETCH FIRST 10 ROWS ONLY"#
    )
    .fetch_all(&mut connection)
    .await?;

    let common = get_common(&mut connection, AdminMenuPages::Dashboard).await?;
    let content = Dashboard {
        common,
        recent_posts,
    };

    render_html(content)
}
