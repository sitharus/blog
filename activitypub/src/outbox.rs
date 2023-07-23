use anyhow::bail;
use cgi::http::header;
use serde_json::Value;
use shared::{
    activities::{Activity, OrderedCollection},
    settings::Settings,
};
use sqlx::{query, PgConnection};

use crate::{actor::get_actor, http_signatures, utils::jsonld_response};

pub async fn render(connection: &mut PgConnection) -> anyhow::Result<cgi::Response> {
    let contents = query!("SELECT activity FROM activitypub_outbox ORDER BY created_at DESC")
        .fetch_all(connection)
        .await?;

    let outbox: OrderedCollection<Activity> = OrderedCollection {
        summary: Some("Outbox".into()),
        items: contents
            .into_iter()
            .map(|i| serde_json::from_value::<Activity>(i.activity).unwrap())
            .collect(),
    };

    jsonld_response(&outbox)
}

pub async fn process(connection: &mut PgConnection, settings: Settings) -> anyhow::Result<String> {
    let to_process = query!(
        r#"
SELECT o.id AS outbox_id, o.activity_id, o.activity, t.target, k.inbox AS "inbox?"
FROM activitypub_outbox o
INNER JOIN activitypub_outbox_target t
ON o.id = t.activitypub_outbox_id
LEFT OUTER JOIN activitypub_known_actors k
ON k.actor = t.target
WHERE o.all_delivered = false
AND t.delivered = false
AND t.retries < 5
ORDER BY t.retries, o.created_at, t.target
"#
    )
    .fetch_all(&mut *connection)
    .await?;

    for row in to_process {
        match send_actvity(
            &mut *connection,
            &row.outbox_id,
            row.activity,
            row.target.clone(),
            row.inbox,
            &settings,
        )
        .await
        {
            Ok(_) => {
                query!("UPDATE activitypub_outbox_target SET delivered=true, delivered_at=CURRENT_TIMESTAMP WHERE activitypub_outbox_id=$1 AND target = $2", row.outbox_id, row.target)
                    .execute(&mut *connection)
                    .await?;
            }
            Err(_) => {
                query!("UPDATE activitypub_outbox_target SET retries = retries + 1 WHERE activitypub_outbox_id=$1 AND target = $2", row.outbox_id, row.target)
                    .execute(&mut *connection)
                    .await?;
            }
        };
    }
    Ok(String::from("Done"))
}

async fn send_actvity(
    connection: &mut PgConnection,
    outbox_id: &i64,
    activity: Value,
    target: String,
    inbox: Option<String>,
    settings: &Settings,
) -> anyhow::Result<()> {
    match get_inbox_for_actor(connection, target.clone(), inbox, settings).await {
        Ok(inbox_uri) => {
            match http_signatures::sign_and_send(
                ureq::post(&inbox_uri)
                    .set(header::CONTENT_TYPE.as_str(), "application/activity+json"),
                activity,
                settings,
            ) {
                Err(a) => match a.downcast::<ureq::Error>() {
                    Ok(ureq::Error::Status(code, response)) => {
                        let status = code.to_string();
                        let body = response.into_string().unwrap_or("--NO BODY--".into());
                        query!("INSERT INTO activitypub_delivery_log(activitypub_outbox_id, target, successful, status_code, response_body) VALUES($1, $2, false, $3, $4)", outbox_id, target, status, body)
                    .execute(&mut *connection)
                    .await?;

                        bail!("Request failed")
                    }
                    Ok(x) => {
                        query!("INSERT INTO activitypub_delivery_log(activitypub_outbox_id, target, successful, response_body) VALUES($1, $2, false, $3)", outbox_id, target, format!("Sending note to {}, {:#}",inbox_uri, x))
                    .execute(&mut *connection)
                    .await?;
                        bail!("Request failed")
                    }
                    Err(x) => {
                        query!("INSERT INTO activitypub_delivery_log(activitypub_outbox_id, target, successful, response_body) VALUES($1, $2, false, $3)", outbox_id, target, format!("Downcasting error {:#}",x))
                    .execute(&mut *connection)
                    .await?;
                        bail!("Request failed")
                    }
                },
                Ok(_) => Ok(()),
            }
        }
        Err(a) => {
            query!("INSERT INTO activitypub_delivery_log(activitypub_outbox_id, target, successful, response_body) VALUES($1, $2, false, $3)", outbox_id, target, format!("Getting inbox: {:#}",a))
                    .execute(&mut *connection)
                    .await?;
            bail!(a)
        }
    }
}
async fn get_inbox_for_actor(
    connection: &mut PgConnection,
    actor: String,
    inbox: Option<String>,
    settings: &Settings,
) -> anyhow::Result<String> {
    match inbox {
        Some(i) => Ok(i),
        None => {
            let known_actor = query!(
                "SELECT inbox FROM activitypub_known_actors WHERE actor=$1",
                actor
            )
            .fetch_optional(&mut *connection)
            .await?;

            match known_actor {
                Some(row) => Ok(row.inbox),
                None => {
                    let actor_record = get_actor(actor, connection, settings).await?;
                    Ok(actor_record.inbox)
                }
            }
        }
    }
}
