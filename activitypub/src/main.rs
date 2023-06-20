use activities::{Activity, OrderedCollection};
use actor::Actor;
use async_std::task;
use cgi::http::{header, response, Uri};
use shared::{
    database::connect_db,
    settings::{get_settings_struct, Settings},
    utils::parse_query_string,
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
        "/activitypub/blog" => actor(&request, fedi_base, settings),
        "/activitypub/inbox" => Ok(cgi::text_response(404, "inbox")),
        "/activitypub/outbox" => outbox(&request, &mut connection).await,
        "/activitypub/followers" => Ok(cgi::text_response(404, "followers")),
        "/activitypub/following" => Ok(cgi::text_response(404, "following")),
        _ => Ok(cgi::text_response(404, "Not found")),
    }
}

fn actor(
    request: &cgi::Request,
    fedi_base: String,
    settings: Settings,
) -> anyhow::Result<cgi::Response> {
    if request.method() == "GET" {
        let actor = Actor::new(
            fedi_base,
            settings.blog_name,
            "blog".into(),
            settings.fedi_public_key_pem,
        );
        jsonld_response(&actor)
    } else {
        Ok(cgi::text_response(400, "Bad request - only GET supported"))
    }
}

async fn outbox(
    request: &cgi::Request,
    connection: &mut PgConnection,
) -> anyhow::Result<cgi::Response> {
    if request.method() == "GET" {
        let items = query!("SELECT id, title FROM posts ORDER BY post_date DESC")
            .fetch_all(connection)
            .await?
            .into_iter()
            .map(|i| {
                Activity::Note(activities::Note {
                    name: i.title,
                    content: "content".into(),
                })
            })
            .collect();

        let outbox: OrderedCollection<Activity> = activities::OrderedCollection {
            summary: "Outbox".into(),
            items,
        };
        jsonld_response(&outbox)
    } else {
        Ok(cgi::text_response(400, "Bad request - only GET supported"))
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
