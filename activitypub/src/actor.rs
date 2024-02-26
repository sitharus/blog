use anyhow::anyhow;
use cgi::http::{header, uri};
use chrono::{DateTime, Utc};
use serde_json::Value;
use shared::settings::Settings;
use sqlx::{query, query_as, PgPool};

use crate::http_signatures::sign_and_call;

pub struct ActorRecord {
    pub id: i64,
    pub first_seen: Option<DateTime<Utc>>,
    pub last_seen: Option<DateTime<Utc>>,
    pub actor: Option<String>,
    pub is_following: bool,
    pub public_key: Option<String>,
    pub inbox: String,
    pub public_key_id: String,
    pub username: Option<String>,
    pub server: Option<String>,
    pub raw_actor_data: Option<Value>,
}

pub async fn get_actor(
    actor_uri: String,
    connection: &PgPool,
    settings: &Settings,
) -> anyhow::Result<ActorRecord> {
    let actor_uri = uri_for_actor(&actor_uri)?;
    let uri_str = actor_uri.as_str();

    let known = query_as!(
        ActorRecord,
        "SELECT * FROM activitypub_known_actors WHERE actor=$1",
        uri_str
    )
    .fetch_optional(connection)
    .await?;
    if let Some(actor) = known {
        Ok(actor)
    } else {
        fetch_actor(uri_str, &actor_uri, connection, settings).await
    }
}

pub async fn refresh_actor(
    actor_uri: String,
    connection: &PgPool,
    settings: &Settings,
) -> anyhow::Result<ActorRecord> {
    let actor_uri = uri_for_actor(&actor_uri)?;
    let uri_str = actor_uri.as_str();
    fetch_actor(uri_str, &actor_uri, connection, settings).await
}

async fn fetch_actor(
    uri_str: &str,
    actor_uri: &String,
    connection: &PgPool,
    settings: &Settings,
) -> anyhow::Result<ActorRecord> {
    let actor_details: Value = sign_and_call(
        ureq::get(uri_str).set(header::ACCEPT.as_str(), "application/jrd+json"),
        settings,
    )
    .map_err(|e| anyhow!("Fetching {}: {:#}", uri_str, e))?
    .into_json()
    .map_err(|e| anyhow!("Parsing JSON from {}: {:#}", uri_str, e))?;

    let inbox = actor_details["inbox"]
        .as_str()
        .ok_or(anyhow!("No inbox in activitypub details for {}", actor_uri))?;
    let public_key = actor_details["publicKey"]["publicKeyPem"].as_str();
    let public_key_id = actor_details["publicKey"]["id"].as_str();
    let username = actor_details["preferred_username"].as_str();
    let server: uri::Uri = uri_str.parse()?;

    let row = query!(
            "
INSERT INTO activitypub_known_actors(is_following, actor, inbox, public_key, public_key_id, username, server, raw_actor_data)
VALUES (false, $1, $2, $3, $4, $5, $6, $7)
ON CONFLICT(actor) DO UPDATE SET public_key=$3, public_key_id=$4, username=$5, server=$6, raw_actor_data=$7
RETURNING id, first_seen, last_seen, is_following
",
            actor_uri,
            inbox,
            public_key,
            public_key_id,
			username,
            server.host(),
            actor_details,
        )
        .fetch_one(connection)
        .await?;

    Ok(ActorRecord {
        id: row.id,
        first_seen: row.first_seen,
        last_seen: row.last_seen,
        actor: Some(uri_str.into()),
        is_following: row.is_following,
        public_key: public_key.map(|s| s.into()),
        inbox: inbox.into(),
        public_key_id: public_key_id.unwrap_or("").into(),
        username: username.map(|u| u.into()),
        server: server.host().map(|s| s.into()),
        raw_actor_data: Some(actor_details),
    })
}

fn uri_for_actor(actor: &String) -> anyhow::Result<String> {
    if actor.starts_with("http") {
        Ok(actor.clone())
    } else {
        let parts: Vec<&str> = actor.split('@').collect();
        if parts.len() == 2 {
            let server = parts[1];
            let finger_uri = format!(
                "https://{}/.well-known/webfinger?resource=acct:{}",
                server, actor
            );

            let finger: Value = ureq::get(&finger_uri)
                .set(header::ACCEPT.as_str(), "application/jrd+json")
                .call()?
                .into_json()?;

            let actor_link = finger["links"]
                .as_array()
                .ok_or(anyhow!("No links in webfinger for {}", actor))?
                .into_iter()
                .find(|x| {
                    x["rel"].as_str() == Some("self")
                        && x["type"].as_str() == Some("application/activity+json")
                })
                .ok_or(anyhow!("Could not find activitypub link for {}", actor))?
                .get("href")
                .ok_or(anyhow!("No href in link element"))?
                .as_str()
                .ok_or(anyhow!("Activitypub link was not a string for {}", actor))?;

            Ok(actor_link.into())
        } else {
            Err(anyhow!("Could not resolve actor from {}", actor))
        }
    }
}
