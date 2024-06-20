use std::collections::HashMap;

use anyhow::{anyhow, bail};
use askama::Template;
use cgi;
use generator::preview_page;
use lazy_static::lazy_static;
use media::manage_media;
use response::{css_response, font_response};
use serde::Deserialize;
use serde_querystring::{from_bytes, ParseMode};
use session::SessionError;
use shared::{
    database,
    utils::{parse_query_string, render_html, render_html_status, render_redirect},
};
use tokio::runtime::Runtime;
use types::PageGlobals;

mod account;
mod activitypub;
mod comments;
mod common;
mod dashboard;
mod filters;
mod generator;
mod links;
mod media;
mod page;
mod post;
mod prepublished;
mod response;
mod session;
mod settings;
mod tags;
mod types;
mod utils;

static ADMIN_CSS: &str = include_str!("../../static/admin.css");
static PLAYFAIR_DISPLAY: &[u8] =
    include_bytes!("../../static/PlayfairDisplay-VariableFont_wght.woff2");
static MONSERATT: &[u8] = include_bytes!("../../static/Montserrat-VariableFont_wght.woff2");
static MONSERATT_ITALIC: &[u8] =
    include_bytes!("../../static/Montserrat-Italic-VariableFont_wght.woff2");

lazy_static! {
    static ref FONTS: HashMap<&'static str, &'static [u8]> = {
        let mut m = HashMap::new();
        m.insert("PlayfairDisplay-VariableFont_wght.woff2", PLAYFAIR_DISPLAY);
        m.insert("Montserrat-VariableFont_wght.woff2", MONSERATT);
        m.insert(
            "Montserrat-Italic-VariableFont_wght.woff2",
            MONSERATT_ITALIC,
        );
        m
    };
}

#[derive(Template)]
#[template(path = "index.html")]
struct Index<'a> {
    username: Option<&'a str>,
    message: Option<&'a str>,
}

#[derive(Template)]
#[template(path = "404.html")]
struct Page404 {}

#[derive(Template)]
#[template(path = "500.html")]
struct Page500 {
    message: String,
}

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

    let db_connection = database::connect_db().await?;
    let single_row = sqlx::query_as!(
        UserRow,
        "SELECT id, password FROM users WHERE username = $1",
        form.username.as_str()
    )
    .fetch_one(&db_connection)
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
                    session::set_session_and_redirect(&db_connection, row.id, "dashboard").await
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
                .execute(&db_connection)
                .await?;

                session::set_session_and_redirect(&db_connection, row.id, "dashboard").await
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

async fn process_inner(
    request: &cgi::Request,
    query_string: &str,
) -> anyhow::Result<cgi::Response> {
    let query: HashMap<String, String> = parse_query_string(query_string)?;

    let action = query
        .get("action")
        .ok_or(anyhow!("No action supplied"))?
        .clone();

    match action.as_str() {
        "login" => do_login(request).await,
        "css" => css_response(ADMIN_CSS),
        "font" => {
            let font_name = query.get("name").ok_or(anyhow!("Font not found"))?;
            let font = FONTS
                .get(font_name.as_str())
                .ok_or(anyhow!("Font not found"))?;
            font_response(font)
        }
        _ => {
            let pool = database::connect_db().await?;
            let session = session::session_id(&pool, &request).await?;
            let default_site_id = "1".to_string();
            let site_id = query
                .get("site")
                .unwrap_or(&default_site_id)
                .to_owned()
                .parse()
                .unwrap();
            let page_request = PageGlobals {
                query,
                site_id,
                connection_pool: pool,
                session,
            };
            match action.as_str() {
                "dashboard" => dashboard::render(&request, page_request).await,
                "new-post" => post::new_post(&request, page_request).await,
                "regenerate" => {
                    generator::regenerate_blog(&page_request).await?;
                    activitypub::publish_posts(page_request, true).await?;
                    render_redirect("dashboard", site_id)
                }
                "account" => account::render(&request, page_request).await,
                "settings" => settings::render(&request, page_request).await,
                "links" => links::render(&request, page_request).await,
                "edit_post" => post::edit_post(&request, page_request).await,
                "manage_posts" => post::manage_posts(page_request).await,
                "comments" => comments::comment_list(page_request).await,
                "moderate_comment" => comments::moderate_comment(&request, page_request).await,
                "preview" => preview_page(&request, page_request).await,
                "manage_pages" => page::manage_pages(page_request).await,
                "new_page" => page::new_page(&request, page_request).await,
                "edit_page" => page::edit_post(&request, page_request).await,
                "media" => manage_media(&request, page_request).await,
                "profile_update" => activitypub::publish_profile_updates(page_request).await,
                "publish_posts" => activitypub::publish_posts_from_request(page_request).await,
                "send_post" => activitypub::send(&request, page_request).await,
                "activitypub_feed" => activitypub::feed(page_request).await,
                "tags" => tags::render(&request, page_request).await,
                "prepublished" => prepublished::prepublished(&request, page_request).await,
                _ => do_404().await,
            }
        }
    }
}

async fn render_500(e: anyhow::Error) -> anyhow::Result<cgi::Response> {
    render_html(Page500 {
        message: format!("{:?}", e),
    })
}

async fn process(request: &cgi::Request, query_string: &str) -> anyhow::Result<cgi::Response> {
    match process_inner(request, query_string).await {
        Err(e) if e.is::<SessionError>() => bail!(e),
        Err(e) => render_500(e).await,
        x => x,
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
        Some(qs) => {
            let runtime = Runtime::new().unwrap();
            match runtime.block_on(process(&request, qs)) {
                Err(e) => redirect_session_error(e),
                x => x
            }
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
