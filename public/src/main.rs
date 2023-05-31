use std::collections::HashMap;

use anyhow::anyhow;
use askama::Template;
use async_std::task;
use cgi;
use shared::{database, generator, utils};
use sqlx::query;

#[derive(Template)]
#[template(path = "400.html")]
struct Http400 {}

#[derive(Template)]
#[template(path = "400.html")]
struct Http404 {}

#[derive(Template)]
#[template(path = "generated/comment_form.html")]
struct CommentForm {
    token: String,
    post_id: i32,
    comment_cgi_url: String,
    static_base_url: String,
}
#[derive(Template)]
#[template(path = "generated/comment_posted.html")]
struct CommentPosted {
    static_base_url: String,
}

#[derive(serde::Deserialize)]
struct NewComment {
    post_id: i32,
    unique_id: String,
    name: String,
    email: String,
    comment: String,
}

async fn comment_form(query: HashMap<String, String>) -> anyhow::Result<cgi::Response> {
    let post_id_str = query.get("post_id").ok_or(anyhow!("No post id"))?;
    let post_id: i32 = post_id_str.parse()?;
    let mut conn = database::connect_db().await?;
    let settings_data = query!("SELECT setting_name, value FROM blog_settings")
        .fetch_all(&mut conn)
        .await?;
    let settings: HashMap<String, String> =
        HashMap::from_iter(settings_data.into_iter().map(|r| (r.setting_name, r.value)));

    query!("SELECT id FROM posts WHERE id=$1", post_id)
        .fetch_one(&mut conn)
        .await?;

    utils::render_html(CommentForm {
        token: "".into(),
        post_id,
        comment_cgi_url: settings.get("comment_cgi_url").unwrap().to_owned(),
        static_base_url: settings.get("static_base_url").unwrap().to_owned(),
    })
}

async fn post_comment(request: &cgi::Request) -> anyhow::Result<cgi::Response> {
    let body: NewComment = utils::post_body(request)?;
    let mut conn = database::connect_db().await?;

    query!(
        "
INSERT INTO comments (post_id, created_date, author_name, author_email, post_body)
VALUES($1, CURRENT_TIMESTAMP, $2, $3, $4)
",
        body.post_id,
        body.name,
        body.email,
        body.comment
    )
    .execute(&mut conn)
    .await?;

    let static_base_url =
        query!("SELECT value FROM blog_settings WHERE setting_name='static_base_url'")
            .fetch_one(&mut conn)
            .await?;

    utils::render_html(CommentPosted {
        static_base_url: static_base_url.value,
    })
}

async fn preview(query: HashMap<String, String>) -> anyhow::Result<cgi::Response> {
    let id: i32 = query.get("id").ok_or(anyhow!("no post ID!"))?.parse()?;

    generator::external_preview(id).await
}

async fn process(request: &cgi::Request, query_string: &str) -> anyhow::Result<cgi::Response> {
    let query: HashMap<String, String> = utils::parse_query_string(query_string)?;
    let action = query.get("action");

    match action {
        Some(str) => match str.as_str() {
            "comment_form" => comment_form(query).await,
            "comment" => post_comment(request).await,
            "preview" => preview(query).await,
            _ => utils::render_html(Http400 {}),
        },
        _ => utils::render_html(Http400 {}),
    }
}

cgi::cgi_try_main! {|request: cgi::Request| -> anyhow::Result<cgi::Response> {

    let maybe_query = request.uri().query();
    match maybe_query {
        Some(qs) =>
            match task::block_on(process(&request, qs)) {
                x => x
            },
        None => {
            utils::render_html(Http404{})
        }
    }
}}
