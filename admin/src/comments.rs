use crate::{
    common::{get_common, Common},
    filters,
    types::{AdminMenuPages, PageGlobals},
};

use askama::Template;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use shared::{
    types::CommentStatus,
    utils::{post_body, render_html, render_redirect},
};
use sqlx::{query, query_as};

#[derive(Template)]
#[template(path = "comment_list.html")]
struct Comments {
    common: Common,
    comments: Vec<CommentListItem>,
}

struct CommentListItem {
    id: i64,
    post_title: String,
    author_name: String,
    author_email: String,
    body: String,
    created_date: DateTime<Utc>,
}

#[derive(Deserialize)]
struct CommentModAction {
    comment_id: i64,
    action: String,
}

pub async fn comment_list(globals: PageGlobals) -> anyhow::Result<cgi::Response> {
    let items = query_as!(
        CommentListItem,
        "
SELECT c.id AS id, p.title AS post_title, c.author_name, c.author_email, c.created_date, c.post_body AS body
FROM comments c
INNER JOIN posts p
ON c.post_id = p.id
WHERE c.status = 'pending'
AND p.site_id=$1
", globals.site_id
    )
    .fetch_all(&globals.connection_pool)
    .await?;

    let common = get_common(&globals, AdminMenuPages::Comments).await?;
    render_html(Comments {
        common,
        comments: items,
    })
}

pub async fn moderate_comment(
    request: &cgi::Request,
    globals: PageGlobals,
) -> anyhow::Result<cgi::Response> {
    let action: CommentModAction = post_body(request)?;
    let status = if action.action == "approve" {
        CommentStatus::Approved
    } else {
        CommentStatus::Spam
    };
    query!(
        "UPDATE comments SET status=$1 WHERE id=$2",
        status as CommentStatus,
        action.comment_id
    )
    .execute(&globals.connection_pool)
    .await?;
    render_redirect("comments")
}
