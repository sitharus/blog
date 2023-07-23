use crate::{
    common::{get_common, Common},
    types::AdminMenuPages,
};
use askama::Template;
use cgi;
use chrono::{DateTime, Utc};
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
    recent_posts: Vec<DashboardPost>,
    followers: Vec<Follower>,
}

struct Follower {
    actor: String,
    first_seen: DateTime<Utc>,
}

struct DashboardPost {
    pub id: i32,
    pub post_date: chrono::NaiveDate,
    pub title: String,
    pub comment_count: i64,
    pub like_count: i64,
}

pub async fn render(request: &cgi::Request) -> anyhow::Result<cgi::Response> {
    let mut connection = database::connect_db().await?;
    session::session_id(&mut connection, &request).await?;

    let recent_posts = query_as!(
        DashboardPost,
        r#"
SELECT id, post_date, title,
       (SELECT COUNT(*) FROM comments WHERE comments.post_id=posts.id) AS "comment_count!",
       (SELECT COUNT(*) FROM activitypub_likes WHERE activitypub_likes.post_id=posts.id) AS "like_count!"
FROM posts
WHERE state = 'published'
ORDER BY post_date DESC
FETCH FIRST 10 ROWS ONLY"#
    )
    .fetch_all(&mut connection)
    .await?;

    let followers = query_as!(
        Follower,
        r#"
SELECT actor AS "actor!", first_seen as "first_seen!"
FROM activitypub_known_actors
WHERE is_following=true AND actor IS NOT NULL
ORDER BY first_seen DESC
"#
    )
    .fetch_all(&mut connection)
    .await?;

    let common = get_common(&mut connection, AdminMenuPages::Dashboard).await?;
    let content = Dashboard {
        common,
        recent_posts,
        followers,
    };

    render_html(content)
}
