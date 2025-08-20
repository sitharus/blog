use crate::actor::get_actor;
use crate::http_signatures::{self, sign_and_call, validate};
use crate::utils::jsonld_response;
use anyhow::bail;
use cgi::http::{Method, header};
use serde_json::Value;
use shared::activities::{Activity, Create, Delete, Follow, Like, Note, OrderedCollection, Undo};
use shared::settings::Settings;
use sqlx::types::Json;
use sqlx::{PgPool, query, query_as};

struct InboxItem {
    message: Option<String>,
    post_id: i32,
    received_at: Option<chrono::DateTime<chrono::Utc>>,
    item_id: Option<String>,
    to: Option<Value>,
    cc: Option<Value>,
    actor: Option<String>,
    object: Option<String>,
}

pub async fn inbox(
    request: &cgi::Request,
    connection: &PgPool,
    settings: &Settings,
) -> anyhow::Result<cgi::Response> {
    match *request.method() {
        Method::GET => {
            let items: Vec<Activity> = match validate(request, connection, settings).await {
                Ok(key) => {
                    let items =
                        query_as!(InboxItem,
                                 "SELECT af.message, al.post_id, ai.received_at, ai.body->>'id' AS item_id, ai.body->'to' AS to , ai.body->'cc' AS cc, ai.body->>'actor' AS actor, ai.body->>'object' AS object
FROM activitypub_known_actors a
LEFT JOIN activitypub_feed af
ON af.actor_id = a.id
LEFT JOIN activitypub_likes al
ON al.actor_id = a.id
LEFT JOIN activitypub_inbox ai ON ai.id IN (af.inbox_item_id, al.inbox_item_id)
WHERE a.public_key_id=$1
ORDER BY ai.received_at DESC
",
                        key
                    )
                        .fetch_all(connection)
                        .await?;

                    items
                        .into_iter()
                        .filter_map(|i| match i {
                            InboxItem {
                                message: Some(message),
                                item_id: Some(item_id),
                                received_at: Some(received_at),
                                to: Some(to),
                                cc,
                                ..
                            } => Some(Activity::Note(Box::new(Note::new(
                                message,
                                item_id,
                                received_at,
                                serde_json::from_value(to).unwrap_or(vec![]),
                                cc.and_then(|c| serde_json::from_value(c).ok())
                                    .unwrap_or(vec![]),
                            )))),
                            InboxItem {
                                message: None,
                                post_id,
                                actor: Some(actor),
                                object: Some(object),
                                ..
                            } if post_id > 0 => {
                                Some(Activity::Like(Box::new(Like { actor, object })))
                            }
                            _ => None,
                        })
                        .collect()
                }
                _ => vec![],
            };
            let inbox: OrderedCollection<Activity> = OrderedCollection {
                items,
                summary: Some("inbox".into()),
                id: None,
            };
            jsonld_response(&inbox)
        }
        Method::POST => {
            let body: Value = serde_json::from_slice(request.body())?;
            http_signatures::validate(request, connection, settings).await?;

            let inserted = query!(
                "INSERT INTO activitypub_inbox(body) VALUES($1) RETURNING id",
                body
            )
            .fetch_one(connection)
            .await?;

            process_inbound(inserted.id, body, connection, settings).await?;

            let following: OrderedCollection<String> = OrderedCollection {
                items: vec![],
                summary: Some("inbox".into()),
                id: None,
            };
            jsonld_response(&following)
        }
        _ => Ok(cgi::text_response(405, "Bad request - only GET supported")),
    }
}

async fn process_inbound(
    inbox_id: i64,
    body: Value,
    connection: &PgPool,
    settings: &Settings,
) -> anyhow::Result<()> {
    let activity: Result<Activity, _> = serde_json::from_value(body);

    match activity {
        Ok(Activity::Follow(req)) => {
            if !is_blocked(req.actor.clone(), connection).await? {
                process_follow(*req, connection, settings).await?;
                mark_as_processed(inbox_id, connection).await
            } else {
                Ok(())
            }
        }
        Ok(Activity::Delete(req)) => {
            process_delete(*req, connection, settings).await?;
            mark_as_processed(inbox_id, connection).await
        }
        Ok(Activity::Undo(undo)) => {
            process_undo(*undo, connection, settings).await?;
            mark_as_processed(inbox_id, connection).await
        }
        Ok(Activity::Create(create)) => {
            if !is_blocked(create.actor.clone(), connection).await? {
                process_create(inbox_id, *create, connection, settings).await?;
            }
            mark_as_processed(inbox_id, connection).await
        }
        Ok(Activity::Like(like)) => {
            process_like(inbox_id, *like, connection, settings).await?;
            mark_as_processed(inbox_id, connection).await
        }
        Err(e) => {
            dbg!("{:?}", e);
            Ok(())
        }
        _ => Ok(()),
    }
}

async fn mark_as_processed(inbox_id: i64, connection: &PgPool) -> anyhow::Result<()> {
    query!(
        "UPDATE activitypub_inbox SET processed=true WHERE id=$1",
        inbox_id
    )
    .execute(connection)
    .await?;
    Ok(())
}

async fn process_follow(
    req: Follow,
    connection: &PgPool,
    settings: &Settings,
) -> anyhow::Result<()> {
    let actor_details: Value = sign_and_call(
        ureq::get(&req.actor).set(header::ACCEPT.as_str(), "application/activity+json"),
        settings,
    )?
    .into_json()?;

    let inbox = actor_details["inbox"].as_str().unwrap();

    let result = query!("INSERT INTO activitypub_known_actors(is_following, actor, public_key, inbox, public_key_id) VALUES (true, $1, $2, $3, $4) ON CONFLICT(actor) DO UPDATE SET is_following=true RETURNING id",
		   &req.actor,
		   actor_details["publicKey"]["publicKeyPem"].as_str(),
		   inbox,
		   actor_details["publicKey"]["id"].as_str()
	)
    .fetch_one(connection)
    .await?;

    query!(
        "INSERT INTO activitypub_followers(site_id, actor_id) VALUES($1, $2)",
        settings.site_id,
        result.id
    )
    .execute(connection)
    .await?;

    let accept = req.accept(settings.activitypub_actor_uri());

    http_signatures::sign_and_send(
        ureq::post(inbox).set(header::CONTENT_TYPE.as_str(), "application/activity+json"),
        accept,
        settings,
    )?;

    Ok(())
}

async fn process_delete(
    req: Delete,
    connection: &PgPool,
    settings: &Settings,
) -> anyhow::Result<()> {
    // Right now only deletes of actors supported.
    let maybe_actor = query!(
        "SELECT id FROM activitypub_known_actors WHERE actor=$1",
        req.object
    )
    .fetch_optional(connection)
    .await?;

    if let Some(actor) = maybe_actor {
        query!(
            "DELETE FROM activitypub_followers WHERE actor_id=$1 AND site_id=$2",
            actor.id,
            settings.site_id
        )
        .execute(connection)
        .await?;
    }
    Ok(())
}

async fn is_blocked(actor: String, connection: &PgPool) -> anyhow::Result<bool> {
    let server = actor.split('@').last().unwrap_or("");
    let result = query!("SELECT COUNT(*) FROM activitypub_blocked WHERE (target_type = 'actor' AND target = $1) OR (target_type = 'server' AND target = $2)", actor, server)
        .fetch_optional(connection)
        .await?;

    Ok(match result {
        Some(row) => row.count.unwrap_or_default() > 0,
        None => false,
    })
}

async fn process_undo(undo: Undo, connection: &PgPool, settings: &Settings) -> anyhow::Result<()> {
    match *undo.object {
        Activity::Follow(follow) => {
            let maybe_actor = query!(
                "SELECT id FROM activitypub_known_actors WHERE actor=$1",
                follow.actor
            )
            .fetch_optional(connection)
            .await?;

            if let Some(actor) = maybe_actor {
                query!(
                    "DELETE FROM activitypub_followers WHERE actor_id=$1 AND site_id=$2",
                    actor.id,
                    settings.site_id
                )
                .execute(connection)
                .await?;
            }
            Ok(())
        }
        _ => bail!("Unknown undo!"),
    }
}

async fn process_create(
    item_id: i64,
    create: Create,
    connection: &PgPool,
    settings: &Settings,
) -> anyhow::Result<()> {
    match create.object() {
        Activity::Note(note) => {
            let actor = get_actor(create.actor.to_owned(), connection, settings).await?;

            query!("INSERT INTO activitypub_feed (actor_id, inbox_item_id, recieved_at, message_timestamp, message, extra_data, site_id) VALUES ($1, $2, CURRENT_TIMESTAMP, $3, $4, $5, $6)",
				   actor.id,
				   item_id,
				   note.published,
				   note.content,
				   Json(note) as _,
                   settings.site_id
)
				.execute(connection)
				.await?;
            Ok(())
        }
        _ => bail!("Unknown create type"),
    }
}

async fn process_like(
    item_id: i64,
    like: Like,
    connection: &PgPool,
    settings: &Settings,
) -> anyhow::Result<()> {
    let actor = get_actor(like.actor.to_owned(), connection, settings).await?;
    let activity = query!(
        "SELECT source_post FROM activitypub_outbox WHERE activity_id=$1",
        like.object
    )
    .fetch_optional(connection)
    .await?;

    if let Some(source) = activity {
        if let Some(source_id) = source.source_post {
            query!(
            "INSERT INTO activitypub_likes(post_id, inbox_item_id, actor_id) VALUES($1, $2, $3)",
            source_id,
            item_id,
            actor.id
        )
            .execute(connection)
            .await?;
        }
    }
    Ok(())
}
