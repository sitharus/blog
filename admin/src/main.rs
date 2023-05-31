use std::collections::HashMap;

use anyhow::anyhow;
use askama::Template;
use async_std::task;
use cgi;
use generator::preview_page;
use serde::Deserialize;
use serde_querystring::{from_bytes, ParseMode};
use session::SessionError;
use shared::{
    database,
    utils::{parse_query_string, render_html, render_html_status},
};

mod account;
mod comments;
mod common;
mod dashboard;
mod filters;
mod generator;
mod links;
mod post;
mod response;
mod session;
mod settings;
mod types;

#[derive(Template)]
#[template(path = "index.html")]
struct Index<'a> {
    username: Option<&'a str>,
    message: Option<&'a str>,
}

#[derive(Template)]
#[template(path = "404.html")]
struct Page404 {}

#[derive(Deserialize)]
struct LoginForm {
    username: String,
    password: String,
}

struct UserRow {
    id: i32,
    password: String,
}

async fn do_login(request: &cgi::Request) -> anyhow::Result<cgi::Response> {
    if request.method() != "POST" {
        return do_404().await;
    }
    let post_items = request.body();
    let form: LoginForm = from_bytes(post_items, ParseMode::UrlEncoded)?;

    let mut db_connection = database::connect_db().await?;
    let single_row = sqlx::query_as!(
        UserRow,
        "SELECT id, password FROM users WHERE username = $1",
        form.username.as_str()
    )
    .fetch_one(&mut db_connection)
    .await;

    fn invalid_user(form: LoginForm) -> anyhow::Result<cgi::Response> {
        let content = Index {
            username: Some(form.username.as_str()),
            message: Some("Invalid username or password"),
        };

        render_html(content)
    }

    match single_row {
        Ok(row) => {
            let dbpassword = row.password;
            let pwbytes = form.password.as_bytes();
            if let Ok(success) = bcrypt::verify(&pwbytes, &dbpassword) {
                if success {
                    session::set_session_and_redirect(&mut db_connection, row.id, "dashboard").await
                } else {
                    invalid_user(form)
                }
            } else if dbpassword == form.password {
                let hashed = bcrypt::hash(dbpassword, bcrypt::DEFAULT_COST)?;
                sqlx::query!(
                    "UPDATE users SET password = $1 WHERE id = $2",
                    hashed,
                    row.id
                )
                .execute(&mut db_connection)
                .await?;

                session::set_session_and_redirect(&mut db_connection, row.id, "dashboard").await
            } else {
                invalid_user(form)
            }
        }
        _ => invalid_user(form),
    }
}

async fn do_404() -> anyhow::Result<cgi::Response> {
    let content = Page404 {};
    render_html_status(404, content)
}

async fn process(request: &cgi::Request, query_string: &str) -> anyhow::Result<cgi::Response> {
    let query: HashMap<String, String> = parse_query_string(query_string)?;

    let action = query.get("action").ok_or(anyhow!("No action supplied"))?;
    if action == "login" {
        do_login(request).await
    } else {
        let mut connection = database::connect_db().await?;
        session::session_id(&mut connection, &request).await?;
        match action.as_str() {
            "dashboard" => dashboard::render(request).await,
            "new-post" => post::new_post(request).await,
            "regenerate" => generator::regenerate_blog(request).await,
            "account" => account::render(request).await,
            "settings" => settings::render(request).await,
            "links" => links::render(request).await,
            "posts" => post::manage_posts(query).await,
            "edit_post" => post::edit_post(request, query).await,
            "comments" => comments::comment_list().await,
            "moderate_comment" => comments::moderate_comment(request).await,
            "preview" => preview_page(request).await,
            _ => do_404().await,
        }
    }
}

fn redirect_session_error(e: anyhow::Error) -> anyhow::Result<cgi::Response> {
    if e.is::<SessionError>() {
        let body: Vec<u8> = "Redirecting".as_bytes().to_vec();
        let resp: cgi::Response = http::response::Builder::new()
            .status(302)
            .header(http::header::LOCATION, "?")
            .header(http::header::SET_COOKIE, "blog_session=; Max-Age=0")
            .header(http::header::CONTENT_TYPE, "text/plain")
            .body(body)?;
        Ok(resp)
    } else {
        Err(e)
    }
}

cgi::cgi_try_main! { |request: cgi::Request| -> anyhow::Result<cgi::Response> {
    let maybe_query = request.uri().query();
    match maybe_query {
        Some(qs) =>
            match task::block_on(process(&request, qs)) {
                Err(e) => redirect_session_error(e),
                x => x
            },
        None => {
            let content = Index {
                username: None,
                message: None
            };
            render_html(content)
        }
    }
}}
