use anyhow::{anyhow, bail};
use cgi::http::{header, response, Uri};
use finger::process_finger;
use shared::{
    activities::{Activity, Actor, OrderedCollection},
    database::connect_db,
    settings::{SettingNames, Settings},
    utils::parse_query_string,
};
use sqlx::{query, PgPool};
use std::{collections::HashMap, env};
use tokio::runtime::Runtime;
use utils::jsonld_response;

use crate::utils::settings_for_actor;

mod actor;
mod finger;
mod http_signatures;
mod inbox;
mod outbox;
mod utils;

fn main() -> Result<(), anyhow::Error> {
    let args: Vec<String> = env::args().collect();
    let runtime = Runtime::new().unwrap();

    if args.len() == 2 {
        match runtime.block_on(cli_run(args)) {
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
                Err(e) => {
                    eprintln!("CGI Error: {:?}", e);
                    cgi::text_response(500, "Internal server error")
                }
            }
        }))
    }
}

async fn cli_run(args: Vec<String>) -> anyhow::Result<String> {
    let connection = connect_db().await?;
    match args[1].as_str() {
        "--process-outbox" => outbox::process(&connection).await,
        _ => bail!("Unknown action"),
    }
}

async fn process(request: cgi::Request) -> anyhow::Result<cgi::Response> {
    let connection = connect_db().await?;
    let original_uri = env::var("REQUEST_URI")?.parse::<Uri>()?;
    let qstr = original_uri.query().unwrap_or("");
    let query_string: HashMap<String, String> = parse_query_string(qstr)?;

    let env_vars: HashMap<String, String> = std::env::vars().collect();
    let server_name = env_vars
        .get("SERVER_NAME")
        .ok_or(anyhow!("No server name?"))?;

    match original_uri.path() {
        "/.well-known/host-meta" => host_meta(&connection, server_name).await,
        "/.well-known/webfinger" => {
            process_finger(request, &connection, server_name, query_string).await
        }
        path if path.starts_with("/activitypub/") => {
            process_activitypub_url(&connection, &request, path, server_name).await
        }

        /*
            "/activitypub/inbox/reprocess" => {
                inbox::reprocess(&request, &query_string, &mut connection, &settings).await
        }
            */

        /*
        "/activitypub/refresh" => {
            has_valid_session(&mut connection, &request).await?;
            refresh_actor(
                query_string.get("actor_uri").unwrap().to_string(),
                &mut connection,
                &settings,
            )
            .await?;
            Ok(cgi::text_response(200, "refreshed"))
        }*/
        _ => {
            eprintln!("Could not find handler for {}", original_uri.path());
            let msg = format!("Not found {}", original_uri.path());
            Ok(cgi::text_response(404, msg))
        }
    }
}

async fn process_activitypub_url(
    connection: &PgPool,
    request: &cgi::Request,
    path: &str,
    hostname: &str,
) -> anyhow::Result<cgi::Response> {
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() < 3 || parts.len() > 4 {
        Ok(cgi::empty_response(404))
    } else if parts.len() == 3 {
        let settings = settings_for_actor(connection, hostname, "blog").await?;
        let action = if parts[2] == "blog" {
            "actor"
        } else {
            parts[2]
        };
        process_activitypub_action(connection, request, settings, action).await
    } else {
        let settings = settings_for_actor(connection, hostname, parts[2]).await?;
        process_activitypub_action(connection, request, settings, parts[3]).await
    }
}

async fn process_activitypub_action(
    connection: &PgPool,
    request: &cgi::Request,
    settings: Settings,
    action: &str,
) -> anyhow::Result<cgi::Response> {
    match action {
        "inbox" => inbox::inbox(request, connection, &settings).await,
        "outbox" => outbox::render(connection, &settings).await,
        "actor" => actor(request, settings),
        "followers" => followers(request, connection, &settings).await,
        "following" => following(request).await,
        _ => Ok(cgi::empty_response(404)),
    }
}

fn actor(request: &cgi::Request, settings: Settings) -> anyhow::Result<cgi::Response> {
    if request.method() == "GET" {
        let actor = Activity::Person(Actor::new(settings));
        jsonld_response(&actor)
    } else {
        Ok(cgi::text_response(405, "Bad request - only GET supported"))
    }
}

async fn followers(
    request: &cgi::Request,
    connection: &PgPool,
    settings: &Settings,
) -> anyhow::Result<cgi::Response> {
    if request.method() == "GET" {
        let followers = query!("SELECT actor FROM activitypub_known_actors ka INNER JOIN activitypub_followers af ON af.actor_id=ka.id WHERE af.site_id=$1", settings.site_id)
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

async fn host_meta(connection: &PgPool, host_name: &String) -> anyhow::Result<cgi::Response> {
    let valid_sites = query!(
        "SELECT value, site_id FROM blog_settings WHERE setting_name=$1 AND value=$2",
        SettingNames::CanonicalHostname.to_string(),
        host_name
    )
    .fetch_all(connection)
    .await?;

    if valid_sites.len() == 0 {
        return Ok(cgi::empty_response(404));
    }

    let body = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<XRD xmlns="http://docs.oasis-open.org/ns/xri/xrd-1.0">
  <Link rel="lrdd" template="https://{}/.well-known/webfinger?resource={{uri}}"/>
</XRD>"#,
        host_name
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
