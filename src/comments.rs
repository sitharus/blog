use crate::{
    database, filters,
    types::{AdminMenuPages, CommentStatus},
    utils::{post_body, render_html, render_redirect},
};
use askama::Template;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::{query, query_as};

#[derive(Template)]
#[template(path = "comment_list.html")]
struct Comments {
    selected_menu_item: AdminMenuPages,
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

pub async fn comment_list() -> anyhow::Result<cgi::Response> {
    let mut db = database::connect_db().await?;
    let items = query_as!(
        CommentListItem,
        "
SELECT c.id AS id, p.title AS post_title, c.author_name, c.author_email, c.created_date, c.post_body AS body
FROM comments c
INNER JOIN posts p
ON c.post_id = p.id
WHERE c.status = 'pending'
"
    )
    .fetch_all(&mut db)
    .await?;

    render_html(Comments {
        selected_menu_item: AdminMenuPages::Comments,
        comments: items,
    })
}

pub async fn moderate_comment(request: &cgi::Request) -> anyhow::Result<cgi::Response> {
    let mut db = database::connect_db().await?;
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
    .execute(&mut db)
    .await?;
    render_redirect("comments")
}
