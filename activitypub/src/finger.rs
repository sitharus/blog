use std::collections::HashMap;

use serde::Serialize;
use sqlx::PgPool;

use crate::{jrd_response, utils::settings_for_actor};

#[derive(Serialize, Debug, Clone)]
pub struct Finger {
    subject: String,
    links: Vec<Link>,
}

#[derive(Serialize, Debug, Clone)]
struct Link {
    rel: String,
    #[serde(rename = "type")]
    link_type: String,
    href: String,
}

impl Finger {
    pub fn new<T: ToString>(user: T, host: T, actor_uri: T) -> Finger {
        let subject = format!("acct:{}@{}", user.to_string(), host.to_string());
        let links = [Link {
            rel: "self".into(),
            link_type: "application/activity+json".into(),
            href: actor_uri.to_string(),
        }]
        .to_vec();
        Finger { subject, links }
    }
}

pub async fn process_finger(
    request: cgi::Request,
    connection: &PgPool,
    server_name: &String,
    query_string: HashMap<String, String>,
) -> anyhow::Result<cgi::Response> {
    if request.method() == "GET" {
        let resource = query_string.get("resource");

        match resource {
            Some(account) => {
                let parts: Vec<&str> = account.split(':').collect();
                if parts.len() != 2 || parts[0] != "acct" {
                    return Ok(cgi::empty_response(400));
                }
                let actor_host: Vec<&str> = parts[1].split('@').collect();
                if actor_host.len() != 2 || actor_host[1] != server_name {
                    return Ok(cgi::empty_response(400));
                }
                let settings = settings_for_actor(connection, actor_host[0], actor_host[1]).await?;

                let finger = Finger::new(
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
