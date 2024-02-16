use crate::{
    common::{get_common, Common},
    types::{AdminMenuPages, PageGlobals},
};
use askama::Template;
use cgi;
use chrono::{DateTime, Utc};
use shared::utils::render_html;
use sqlx::query_as;
use std::fmt;

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
    username: Option<String>,
    server: Option<String>,
    first_seen: DateTime<Utc>,
}

impl fmt::Display for Follower {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(server) = &self.server {
            if let Some(username) = &self.username {
                write!(f, "{}@{}", username, server)
            } else {
                match self.actor.split('/').last() {
                    Some(name) => write!(f, "{}@{}", name, server),
                    _ => write!(f, "{}", self.actor),
                }
            }
        } else {
            write!(f, "{}", self.actor)
        }
    }
}

struct DashboardPost {
    pub id: i32,
    pub post_date: chrono::NaiveDate,
    pub title: String,
    pub comment_count: i64,
    pub like_count: i64,
}

pub async fn render(request: &cgi::Request, globals: PageGlobals) -> anyhow::Result<cgi::Response> {
    session::session_id(&globals.connection_pool, &request).await?;

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
    .fetch_all(&globals.connection_pool)
    .await?;

    let followers = query_as!(
        Follower,
        r#"
SELECT actor AS "actor!", username, server, first_seen as "first_seen!"
FROM activitypub_known_actors
WHERE is_following=true AND actor IS NOT NULL
ORDER BY first_seen DESC
"#
    )
    .fetch_all(&globals.connection_pool)
    .await?;

    let common = get_common(&globals, AdminMenuPages::Dashboard).await?;
    let content = Dashboard {
        common,
        recent_posts,
        followers,
    };

    render_html(content)
}
