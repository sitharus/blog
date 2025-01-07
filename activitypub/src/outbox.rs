use anyhow::bail;
use cgi::http::header;
use serde_json::Value;
use shared::{
    activities::{Activity, OrderedCollection},
    settings::{get_settings_struct, Settings},
};
use sqlx::{query, PgPool};

use crate::{
    actor::get_actor,
    http_signatures,
    utils::{format_activitypub_url, jsonld_response},
};

pub async fn render(connection: &PgPool, settings: &Settings) -> anyhow::Result<cgi::Response> {
    let contents = query!(
        "SELECT id, activity FROM activitypub_outbox WHERE site_id=$1 ORDER BY created_at DESC",
        settings.site_id
    )
    .fetch_all(connection)
    .await?;

    let outbox: OrderedCollection<Activity> = OrderedCollection {
        id: Some(format_activitypub_url("outbox", settings)),
        summary: Some("Outbox".into()),
        items: contents
            .into_iter()
            .map(|i| {
                serde_json::from_value::<Activity>(i.activity)
                    .expect(format!("Not an activity: {:?}", i.id).as_str())
            })
            .collect(),
    };

    jsonld_response(&outbox)
}

pub async fn process(connection: &PgPool) -> anyhow::Result<String> {
    let to_process = query!(
        r#"
SELECT o.id AS outbox_id, o.activity_id, o.activity, t.target, k.inbox AS "inbox?", site_id
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
    .fetch_all(connection)
    .await?;

    for row in to_process {
        let settings = get_settings_struct(connection, row.site_id).await?;
        match send_actvity(
            connection,
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
                    .execute(connection)
                    .await?;
            }
            Err(_) => {
                query!("UPDATE activitypub_outbox_target SET retries = retries + 1 WHERE activitypub_outbox_id=$1 AND target = $2", row.outbox_id, row.target)
                    .execute(connection)
                    .await?;
            }
        };
    }
    Ok(String::from("Done"))
}

async fn send_actvity(
    connection: &PgPool,
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
                    .execute(connection)
                    .await?;

                        bail!("Request failed")
                    }
                    Ok(x) => {
                        query!("INSERT INTO activitypub_delivery_log(activitypub_outbox_id, target, successful, response_body) VALUES($1, $2, false, $3)", outbox_id, target, format!("Sending note to {}, {:#}",inbox_uri, x))
                    .execute(connection)
                    .await?;
                        bail!("Request failed")
                    }
                    Err(x) => {
                        query!("INSERT INTO activitypub_delivery_log(activitypub_outbox_id, target, successful, response_body) VALUES($1, $2, false, $3)", outbox_id, target, format!("Downcasting error {:#}",x))
                    .execute(connection)
                    .await?;
                        bail!("Request failed")
                    }
                },
                Ok(_) => Ok(()),
            }
        }
        Err(a) => {
            query!("INSERT INTO activitypub_delivery_log(activitypub_outbox_id, target, successful, response_body) VALUES($1, $2, false, $3)", outbox_id, target, format!("Getting inbox: {:#}",a))
                    .execute(connection)
                    .await?;
            bail!(a)
        }
    }
}
async fn get_inbox_for_actor(
    connection: &PgPool,
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
            .fetch_optional(connection)
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
