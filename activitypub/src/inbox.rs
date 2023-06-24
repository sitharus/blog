use std::collections::HashMap;

use crate::activities::{Follow, OrderedCollection};
use crate::http_signatures;
use crate::utils::jsonld_response;
use anyhow::anyhow;
use cgi::http::{header, Method};
use serde_json::Value;
use shared::settings::Settings;
use sqlx::{query, PgConnection};

pub async fn inbox(
    request: &cgi::Request,
    connection: &mut PgConnection,
    settings: &Settings,
) -> anyhow::Result<cgi::Response> {
    match request.method() {
        &Method::GET => {
            let following: OrderedCollection<String> = OrderedCollection {
                items: vec![],
                summary: "Followers".into(),
            };
            jsonld_response(&following)
        }
        &Method::POST => {
            let mut body: Value = serde_json::from_slice(request.body())?;
            let signature = request.headers().get("Signature");
            let digest = request.headers().get("Digest");
            body["http_signature"] = match signature {
                Some(val) => serde_json::Value::String(val.to_str().unwrap_or("").into()),
                _ => Value::Null,
            };
            body["digest"] = match digest {
                Some(val) => serde_json::Value::String(val.to_str().unwrap_or("").into()),
                _ => Value::Null,
            };

            http_signatures::validate(request).await?;

            query!("INSERT INTO activitypub_inbox(body) VALUES($1)", body)
                .execute(&mut *connection)
                .await?;

            process_inbound(body, &mut *connection, settings).await?;

            let following: OrderedCollection<String> = OrderedCollection {
                items: vec![],
                summary: "Followers".into(),
            };
            jsonld_response(&following)
        }
        _ => Ok(cgi::text_response(405, "Bad request - only GET supported")),
    }
}

pub async fn reprocess(
    query_string: &HashMap<String, String>,
    connection: &mut PgConnection,
    settings: &Settings,
) -> anyhow::Result<cgi::Response> {
    let id_str = query_string
        .get("id")
        .ok_or(anyhow!("Id must be supplied"))?;
    let id: i64 = id_str.parse()?;
    let row = query!("SELECT body FROM activitypub_inbox WHERE id=$1", id)
        .fetch_one(&mut *connection)
        .await?;

    if let Some(body) = row.body {
        process_inbound(body, connection, settings).await?;
    }

    Ok(cgi::text_response(200, "Done"))
}

async fn process_inbound(
    body: Value,
    connection: &mut PgConnection,
    settings: &Settings,
) -> anyhow::Result<()> {
    match body["type"].as_str() {
        Some("Follow") => {
            let req: Follow = serde_json::from_value(body)?;
            process_follow(req, connection, settings).await
        }
        _ => Ok(()),
    }
}

async fn process_follow(
    req: Follow,
    connection: &mut PgConnection,
    settings: &Settings,
) -> anyhow::Result<()> {
    let actor_details: Value = ureq::get(&req.actor)
        .set(header::ACCEPT.as_str(), "application/activity+json")
        .call()?
        .into_json()?;

    let inbox = actor_details["inbox"].as_str().unwrap();

    query!("INSERT INTO activitypub_known_actors(is_following, actor, public_key, inbox, public_key_id) VALUES (true, $1, $2, $3, $4) ON CONFLICT(actor) DO UPDATE SET is_following=true",
		   &req.actor,
		   actor_details["publicKey"]["publicKeyPem"].as_str(),
		   inbox,
		   actor_details["publicKey"]["id"].as_str()
	)
		.execute(&mut *connection)
		.await?;

    let accept = req.accept(settings.activitypub_actor_uri());

    http_signatures::sign_and_send(
        ureq::post(&inbox).set(header::CONTENT_TYPE.as_str(), "application/activity+json"),
        accept,
        settings,
    )?;
    Ok(())
}
