use crate::types::{AdminMenuPages, Post};
use crate::utils::render_html;
use askama::Template;
use cgi;
use sqlx::query_as;

use super::database;
use super::session;
use crate::filters;

#[derive(Template)]
#[template(path = "dashboard.html")]
struct Dashboard {
    selected_menu_item: AdminMenuPages,
    recent_posts: Vec<Post>,
}

pub async fn render(request: &cgi::Request) -> anyhow::Result<cgi::Response> {
    let mut connection = database::connect_db().await?;
    session::session_id(&mut connection, &request).await?;

    let recent_posts = query_as!(
        Post,
        "SELECT * FROM posts ORDER BY post_date DESC FETCH FIRST 10 ROWS ONLY"
    )
    .fetch_all(&mut connection)
    .await?;
    let content = Dashboard {
        selected_menu_item: AdminMenuPages::Dashboard,
        recent_posts,
    };

    render_html(content)
}
