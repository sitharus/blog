use activities::{Activity, OrderedCollection};
use actor::Actor;
use async_std::task;
use cgi::http::{header, response, Method, Uri};
use shared::{
    database::connect_db,
    settings::{get_settings_struct, Settings},
    utils::{blog_post_url, parse_query_string},
};
use sqlx::{query, PgConnection};
use std::{collections::HashMap, env};

mod activities;
mod actor;
mod finger;

cgi::cgi_try_main! { |request: cgi::Request| -> anyhow::Result<cgi::Response> {
    task::block_on(process(request))
}}

async fn process(request: cgi::Request) -> anyhow::Result<cgi::Response> {
    let mut connection = connect_db().await?;
    let original_uri = env::var("REQUEST_URI")?.parse::<Uri>()?;
    let qstr = original_uri.query().unwrap_or("");
    let query_string: HashMap<String, String> = parse_query_string(qstr)?;

    let settings = get_settings_struct(&mut connection).await?;

    let actor_name = "blog";

    let fedi_base = format!("https://{}/activitypub/", settings.canonical_hostname);
    let actor_uri = format!("{}{}", fedi_base, actor_name);

    match original_uri.path() {
        "/.well-known/host-meta" => host_meta(&settings),
        "/.well-known/webfinger" => {
            if request.method() == "GET" {
                let resource = query_string.get("resource");
                let account = format!("acct:{}@{}", actor_name, settings.canonical_hostname);

                match resource {
                    Some(acct) if acct == &account => {
                        let finger = finger::Finger::new(
                            actor_name.into(),
                            settings.canonical_hostname,
                            actor_uri,
                        );
                        jrd_response(&finger)
                    }

                    _ => Ok(cgi::text_response(404, "Not found")),
                }
            } else {
                Ok(cgi::text_response(400, "Bad request - only GET supported"))
            }
        }
        "/activitypub/blog" => actor(&request, actor_name.into(), fedi_base, settings),
        "/activitypub/inbox" => inbox(&request).await,
        "/activitypub/outbox" => outbox(&request, &mut connection, settings).await,
        "/activitypub/followers" => followers(&request).await,
        "/activitypub/following" => following(&request).await,
        _ => Ok(cgi::text_response(404, "Not found")),
    }
}

fn actor(
    request: &cgi::Request,
    actor_name: String,
    fedi_base: String,
    settings: Settings,
) -> anyhow::Result<cgi::Response> {
    if request.method() == "GET" {
        let actor = Actor::new(fedi_base, actor_name, "blog".into(), settings);
        jsonld_response(&actor)
    } else {
        Ok(cgi::text_response(405, "Bad request - only GET supported"))
    }
}

async fn inbox(request: &cgi::Request) -> anyhow::Result<cgi::Response> {
    match request.method() {
        &Method::GET => {
            let following: OrderedCollection<String> = activities::OrderedCollection {
                items: vec![],
                summary: "Followers".into(),
            };
            jsonld_response(&following)
        }
        &Method::POST => {
            let following: OrderedCollection<String> = activities::OrderedCollection {
                items: vec![],
                summary: "Followers".into(),
            };
            jsonld_response(&following)
        }
        _ => Ok(cgi::text_response(405, "Bad request - only GET supported")),
    }
}

async fn outbox(
    request: &cgi::Request,
    connection: &mut PgConnection,
    settings: Settings,
) -> anyhow::Result<cgi::Response> {
    if request.method() == "GET" {
        let items =
            query!("SELECT id, title, url_slug, post_date FROM posts WHERE state='published' ORDER BY post_date DESC")
                .fetch_all(connection)
                .await?
                .into_iter()
                .map(|i| {
                    let url =
                        blog_post_url(i.url_slug, i.post_date, settings.base_url.clone()).unwrap();
                    Activity::Note(activities::Note {
                        name: i.title.clone(),
                        content: format!(r#"New blog post! <a href="{}">{}</a>"#, url, i.title),
                        id: url,
                    })
                })
                .collect();

        let outbox: OrderedCollection<Activity> = activities::OrderedCollection {
            summary: "Outbox".into(),
            items,
        };
        jsonld_response(&outbox)
    } else {
        Ok(cgi::text_response(405, "Bad request - only GET supported"))
    }
}

async fn followers(request: &cgi::Request) -> anyhow::Result<cgi::Response> {
    if request.method() == "GET" {
        let following: OrderedCollection<String> = activities::OrderedCollection {
            items: vec![],
            summary: "Followers".into(),
        };
        jsonld_response(&following)
    } else {
        Ok(cgi::text_response(405, "Bad request - only GET supported"))
    }
}

async fn following(request: &cgi::Request) -> anyhow::Result<cgi::Response> {
    if request.method() == "GET" {
        let following: OrderedCollection<String> = activities::OrderedCollection {
            items: vec![],
            summary: "Following".into(),
        };
        jsonld_response(&following)
    } else {
        Ok(cgi::text_response(405, "Bad request - only GET supported"))
    }
}

pub fn jrd_response<T>(content: &T) -> anyhow::Result<cgi::Response>
where
    T: ?Sized + serde::Serialize,
{
    let body = serde_json::to_vec(content)?;
    let response = response::Builder::new()
        .status(200)
        .header(header::CONTENT_LENGTH, format!("{}", body.len()).as_str())
        .header(header::CONTENT_TYPE, "application/jrd+json")
        .body(body)?;
    Ok(response)
}

pub fn jsonld_response<T>(content: &T) -> anyhow::Result<cgi::Response>
where
    T: ?Sized + serde::Serialize,
{
    let body = serde_json::to_vec(content)?;
    let response = response::Builder::new()
        .status(200)
        .header(header::CONTENT_LENGTH, format!("{}", body.len()).as_str())
        .header(
            header::CONTENT_TYPE,
            r#"application/ld+json; profile="https://www.w3.org/ns/activitystreams""#,
        )
        .body(body)?;
    Ok(response)
}

fn host_meta(settings: &Settings) -> anyhow::Result<cgi::Response> {
    let body = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<XRD xmlns="http://docs.oasis-open.org/ns/xri/xrd-1.0">
  <Link rel="lrdd" template="https://{}/.well-known/webfinger?resource={{uri}}"/>
</XRD>"#,
        settings.canonical_hostname
    );

    let body_content = body.as_bytes().to_vec();

    Ok(response::Builder::new()
        .status(200)
        .header(
            header::CONTENT_LENGTH,
            format!("{}", body_content.len()).as_str(),
        )
        .header(
            header::CONTENT_TYPE,
            r#"application/xrd+xml; charset=utf-8"#,
        )
        .body(body_content)?)
}

/*
OrderedCollection:


{
  "@context": "https://www.w3.org/ns/activitystreams",
  "summary": "Sally's notes",
  "type": "OrderedCollection",
  "totalItems": 2,
  "orderedItems": [
    {
      "type": "Note",
      "name": "A Simple Note"
    },
    {
      "type": "Note",
      "name": "Another Simple Note"
    }
  ]
}

*/
