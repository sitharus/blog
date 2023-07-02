use anyhow::anyhow;
use cgi::http::header;
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use shared::settings::Settings;
use sqlx::{query, query_as, PgConnection};

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
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Actor {
    #[serde(rename = "@context")]
    context: Vec<String>,
    id: String,
    #[serde(rename = "type")]
    actor_type: String,
    preferred_username: String,
    inbox: String,
    outbox: String,
    followers: String,
    following: String,
    public_key: PublicKey,
    name: String,
    url: String,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct PublicKey {
    id: String,
    owner: String,
    public_key_pem: String,
}

impl Actor {
    pub fn new(settings: Settings) -> Actor {
        let fedi_base = settings.activitypub_base();
        let actor_name = settings.actor_name.clone();
        let id = settings.activitypub_actor_uri();
        let owner_id = settings.activitypub_actor_uri();
        let key_id = settings.activitypub_key_id();
        Actor {
            context: vec![
                "https://www.w3.org/ns/activitystreams".into(),
                "https://w3id.org/security/v1".into(),
            ],
            id,
            actor_type: "Person".into(),
            preferred_username: actor_name,
            inbox: format!("{}inbox", fedi_base),
            outbox: format!("{}outbox", fedi_base),
            followers: format!("{}followers", fedi_base),
            following: format!("{}following", fedi_base),
            public_key: PublicKey {
                id: key_id,
                owner: owner_id,
                public_key_pem: settings.fedi_public_key_pem,
            },
            name: settings.blog_name,
            url: settings.base_url,
        }
    }
}

pub async fn get_actor(
    actor_uri: String,
    connection: &mut PgConnection,
    settings: &Settings,
) -> anyhow::Result<ActorRecord> {
    let actor_uri = uri_for_actor(&actor_uri)?;
    let uri_str = actor_uri.as_str();

    let known = query_as!(
        ActorRecord,
        "SELECT * FROM activitypub_known_actors WHERE actor=$1",
        uri_str
    )
    .fetch_optional(&mut *connection)
    .await?;
    if let Some(actor) = known {
        Ok(actor)
    } else {
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

        let row = query!(
            "
INSERT INTO activitypub_known_actors(is_following, actor, inbox, public_key, public_key_id)
VALUES (false, $1, $2, $3, $4)
ON CONFLICT(actor) DO NOTHING
RETURNING id
",
            actor_uri,
            inbox,
            public_key,
            public_key_id
        )
        .fetch_one(connection)
        .await?;

        Ok(ActorRecord {
            id: row.id,
            first_seen: Some(Utc::now()),
            last_seen: Some(Utc::now()),
            actor: Some(uri_str.into()),
            is_following: false,
            public_key: public_key.map(|s| s.into()),
            inbox: inbox.into(),
            public_key_id: public_key_id.unwrap_or("").into(),
            username: None,
            server: None,
        })
    }
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
