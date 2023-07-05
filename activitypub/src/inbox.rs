use std::collections::HashMap;

use crate::actor::get_actor;
use crate::http_signatures::{self, sign_and_call};
use crate::utils::jsonld_response;
use anyhow::{anyhow, bail};
use cgi::http::{header, Method};
use serde_json::Value;
use shared::activities::{Activity, Create, Delete, Follow, OrderedCollection, Undo};
use shared::session::has_valid_session;
use shared::settings::Settings;
use sqlx::types::Json;
use sqlx::{query, PgConnection};

pub async fn inbox(
    request: &cgi::Request,
    connection: &mut PgConnection,
    settings: &Settings,
) -> anyhow::Result<cgi::Response> {
    match request.method() {
        &Method::GET => {
            let inbox: OrderedCollection<String> = OrderedCollection {
                items: vec![],
                summary: Some("inbox".into()),
            };
            jsonld_response(&inbox)
        }
        &Method::POST => {
            let body: Value = serde_json::from_slice(request.body())?;
            http_signatures::validate(request).await?;

            let inserted = query!(
                "INSERT INTO activitypub_inbox(body) VALUES($1) RETURNING id",
                body
            )
            .fetch_one(&mut *connection)
            .await?;

            process_inbound(inserted.id, body, &mut *connection, settings).await?;

            let following: OrderedCollection<String> = OrderedCollection {
                items: vec![],
                summary: Some("inbox".into()),
            };
            jsonld_response(&following)
        }
        _ => Ok(cgi::text_response(405, "Bad request - only GET supported")),
    }
}

pub async fn reprocess(
    request: &cgi::Request,
    query_string: &HashMap<String, String>,
    connection: &mut PgConnection,
    settings: &Settings,
) -> anyhow::Result<cgi::Response> {
    has_valid_session(connection, request).await?;
    let id_str = query_string
        .get("id")
        .ok_or(anyhow!("Id must be supplied"))?;
    let id: i64 = id_str.parse()?;
    let row = query!(
        "SELECT body FROM activitypub_inbox WHERE id=$1 and processed = false",
        id
    )
    .fetch_one(&mut *connection)
    .await?;

    if let Some(body) = row.body {
        process_inbound(id, body, connection, settings).await?;
    }

    Ok(cgi::text_response(200, "Done"))
}

async fn process_inbound(
    inbox_id: i64,
    body: Value,
    connection: &mut PgConnection,
    settings: &Settings,
) -> anyhow::Result<()> {
    let activity: Result<Activity, _> = serde_json::from_value(body);

    match activity {
        Ok(Activity::Follow(req)) => {
            if !is_blocked(req.actor.clone(), connection).await? {
                process_follow(req, connection, settings).await?;
                mark_as_processed(inbox_id, connection).await
            } else {
                Ok(())
            }
        }
        Ok(Activity::Delete(req)) => {
            process_delete(req, connection).await?;
            mark_as_processed(inbox_id, connection).await
        }
        Ok(Activity::Undo(undo)) => {
            process_undo(undo, connection).await?;
            mark_as_processed(inbox_id, connection).await
        }
        Ok(Activity::Create(create)) => {
            if !is_blocked(create.actor.clone(), connection).await? {
                process_create(inbox_id, create, connection, settings).await?;
            }
            mark_as_processed(inbox_id, connection).await
        }
        e => {
            dbg!("{:?}", e);
            Ok(())
        }
    }
}

async fn mark_as_processed(inbox_id: i64, connection: &mut PgConnection) -> anyhow::Result<()> {
    query!(
        "UPDATE activitypub_inbox SET processed=true WHERE id=$1",
        inbox_id
    )
    .execute(&mut *connection)
    .await?;
    Ok(())
}

async fn process_follow(
    req: Follow,
    connection: &mut PgConnection,
    settings: &Settings,
) -> anyhow::Result<()> {
    let actor_details: Value = sign_and_call(
        ureq::get(&req.actor).set(header::ACCEPT.as_str(), "application/activity+json"),
        settings,
    )?
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

async fn process_delete(req: Delete, connection: &mut PgConnection) -> anyhow::Result<()> {
    // Right now only deletes of actors supported.
    query!(
        "UPDATE activitypub_known_actors SET is_following=false WHERE actor=$1",
        req.object
    )
    .execute(connection)
    .await?;
    Ok(())
}

async fn is_blocked(actor: String, connection: &mut PgConnection) -> anyhow::Result<bool> {
    let server = actor.split('@').last().unwrap_or("");
    let result = query!("SELECT COUNT(*) FROM activitypub_blocked WHERE (target_type = 'actor' AND target = $1) OR (target_type = 'server' AND target = $2)", actor, server)
        .fetch_optional(connection)
        .await?;

    Ok(match result {
        Some(row) => row.count.unwrap_or_default() > 0,
        None => false,
    })
}

async fn process_undo(undo: Undo, connection: &mut PgConnection) -> anyhow::Result<()> {
    match *undo.object {
        Activity::Follow(follow) => {
            query!(
                "UPDATE activitypub_known_actors SET is_following=false WHERE actor=$1",
                follow.actor
            )
            .execute(connection)
            .await?;
            Ok(())
        }
        _ => bail!("Unknown undo!"),
    }
}

async fn process_create(
    item_id: i64,
    create: Create,
    connection: &mut PgConnection,
    settings: &Settings,
) -> anyhow::Result<()> {
    match create.object() {
        Activity::Note(note) => {
            let actor = get_actor(create.actor.to_owned(), connection, settings).await?;

            query!("INSERT INTO activitypub_feed (actor_id, inbox_item_id, recieved_at, message_timestamp, message, extra_data) VALUES ($1, $2, CURRENT_TIMESTAMP, $3, $4, $5)",
				   actor.id,
				   item_id,
				   note.published,
				   note.content,
				   Json(note) as _
)
				.execute(connection)
				.await?;
            Ok(())
        }
        _ => bail!("Unknown create type"),
    }
}
