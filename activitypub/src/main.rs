use actor::{refresh_actor, Actor};
use anyhow::bail;
use cgi::http::{header, response, Uri};
use shared::{
    activities::OrderedCollection,
    database::connect_db,
    session::has_valid_session,
    settings::{get_settings_struct, Settings},
    utils::parse_query_string,
};
use sqlx::{query, PgConnection};
use std::{collections::HashMap, env};
use tokio::runtime::Runtime;
use utils::jsonld_response;

mod actor;
mod finger;
mod http_signatures;
mod inbox;
mod outbox;
mod utils;

fn main() -> Result<(), anyhow::Error> {
    let args: Vec<String> = env::args().collect();
    let runtime = Runtime::new().unwrap();

    if args.len() == 2 && args[1] == "--process-outbox" {
        match runtime.block_on(cli_run()) {
            Ok(a) => {
                println!("{}", a);
                Ok(())
            }
            Err(e) => bail!(e),
        }
    } else {
        Ok(cgi::handle(|request: cgi::Request| -> cgi::Response {
            match runtime.block_on(process(request)) {
                Ok(a) => a,
                Err(_) => cgi::text_response(500, "Internal server error"),
            }
        }))
    }
}

async fn cli_run() -> anyhow::Result<String> {
    let mut connection = connect_db().await?;
    let settings = get_settings_struct(&mut connection).await?;
    outbox::process(&mut connection, settings).await
}

async fn process(request: cgi::Request) -> anyhow::Result<cgi::Response> {
    let mut connection = connect_db().await?;
    let original_uri = env::var("REQUEST_URI")?.parse::<Uri>()?;
    let qstr = original_uri.query().unwrap_or("");
    let query_string: HashMap<String, String> = parse_query_string(qstr)?;

    let settings = get_settings_struct(&mut connection).await?;

    match original_uri.path() {
        "/.well-known/host-meta" => host_meta(&settings),
        "/.well-known/webfinger" => {
            if request.method() == "GET" {
                let resource = query_string.get("resource");
                let account = format!(
                    "acct:{}@{}",
                    settings.actor_name, settings.canonical_hostname
                );

                match resource {
                    Some(acct) if acct == &account => {
                        let finger = finger::Finger::new(
                            &settings.actor_name,
                            &settings.canonical_hostname,
                            &settings.activitypub_actor_uri(),
                        );
                        jrd_response(&finger)
                    }

                    _ => Ok(cgi::text_response(404, "Not found")),
                }
            } else {
                Ok(cgi::text_response(400, "Bad request - only GET supported"))
            }
        }
        "/activitypub/blog" => actor(&request, settings),
        "/activitypub/inbox" => inbox::inbox(&request, &mut connection, &settings).await,
        "/activitypub/inbox/reprocess" => {
            inbox::reprocess(&request, &query_string, &mut connection, &settings).await
        }
        "/activitypub/outbox" => outbox::render(&mut connection).await,
        "/activitypub/followers" => followers(&request, &mut connection).await,
        "/activitypub/following" => following(&request).await,
        "/activitypub/refresh" => {
            has_valid_session(&mut connection, &request).await?;
            refresh_actor(
                query_string.get("actor_uri").unwrap().to_string(),
                &mut connection,
                &settings,
            )
            .await?;
            Ok(cgi::text_response(200, "refreshed"))
        }
        _ => Ok(cgi::text_response(404, "Not found")),
    }
}

fn actor(request: &cgi::Request, settings: Settings) -> anyhow::Result<cgi::Response> {
    if request.method() == "GET" {
        let actor = Actor::new(settings);
        jsonld_response(&actor)
    } else {
        Ok(cgi::text_response(405, "Bad request - only GET supported"))
    }
}

async fn followers(
    request: &cgi::Request,
    connection: &mut PgConnection,
) -> anyhow::Result<cgi::Response> {
    if request.method() == "GET" {
        let followers =
            query!("SELECT actor FROM activitypub_known_actors WHERE is_following=true")
                .fetch_all(connection)
                .await?;
        let followers_collection: OrderedCollection<String> = OrderedCollection {
            items: followers.into_iter().map(|f| f.actor.unwrap()).collect(),
            summary: Some("Followers".into()),
        };
        jsonld_response(&followers_collection)
    } else {
        Ok(cgi::text_response(405, "Bad request - only GET supported"))
    }
}

async fn following(request: &cgi::Request) -> anyhow::Result<cgi::Response> {
    if request.method() == "GET" {
        let following: OrderedCollection<String> = OrderedCollection {
            items: vec![],
            summary: Some("Following".into()),
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
