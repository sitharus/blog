use std::collections::HashMap;

use askama::Template;
use async_std::task;
use cgi;

mod utils;

#[derive(Template)]
#[template(path = "400.html")]
struct Http400 {}

#[derive(Template)]
#[template(path = "400.html")]
struct Http404 {}

async fn comment_form() -> anyhow::Result<cgi::Response> {
    Ok(cgi::string_response(200, "test"))
}

async fn process(request: &cgi::Request, query_string: &str) -> anyhow::Result<cgi::Response> {
    let query: HashMap<String, String> = utils::parse_query_string(query_string)?;
    let action = query.get("action");

    match action {
        Some(str) => match str.as_str() {
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
