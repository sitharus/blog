use std::collections::HashMap;

use anyhow::{anyhow, bail};
use askama::Template;
use chrono::{DateTime, Utc};
use http::Method;
use shared::{
    activities::{self, Activity, Actor, Update},
    settings::get_settings_struct,
    utils::{blog_post_url, post_body, render_html, render_redirect},
};
use sqlx::{query, query_as, types::Json};
use uuid::Uuid;

use crate::filters;
use crate::{
    common::{get_common, Common},
    types::PageGlobals,
};

#[derive(Template)]
#[template(path = "activitypub_send_post.html")]
struct SendPage {
    message: String,
    common: Common,
}

struct FeedMessage {
    actor: Option<String>,
    message: Option<String>,
    timestamp: DateTime<Utc>,
}

#[derive(Template)]
#[template(path = "activitypub_feed.html")]
struct FeedPage {
    messages: Vec<FeedMessage>,
    common: Common,
}
pub async fn publish_posts_from_request(globals: PageGlobals) -> anyhow::Result<cgi::Response> {
    let push = match globals.query.get("push").map(|f| f.as_str()) {
        Some("true") => true,
        Some(_) => false,
        None => false,
    };
    publish_posts(globals, push).await?;
    render_redirect("dashboard")
}

pub async fn publish_posts(globals: PageGlobals, push: bool) -> anyhow::Result<()> {
    let follower_rows =
        query!("SELECT actor FROM activitypub_known_actors aka WHERE EXISTS (SELECT 1 FROM activitypub_followers af WHERE af.actor_id = aka.id AND af.site_id=$1)", globals.site_id)
            .fetch_all(&globals.connection_pool)
            .await?;
    let followers: Vec<String> = follower_rows
        .into_iter()
        .map(|r| r.actor.unwrap())
        .collect();

    let to_post = query!("SELECT id, title, summary, url_slug, post_date FROM posts p WHERE p.site_id = $1 AND NOT EXISTS (SELECT 1 FROM activitypub_outbox o WHERE o.source_post = p.id) AND p.state = 'published'", globals.site_id)
        .fetch_all(&globals.connection_pool)
        .await?;

    let settings = get_settings_struct(&globals.connection_pool, globals.site_id).await?;

    for post in to_post {
        let post_url = blog_post_url(post.url_slug, post.post_date, settings.base_url.clone())?;

        let summary = post.summary.unwrap_or("New post!".into());
        let content = format!(
            r#"<p>{}</p><p><a href="{}">{}</a></p>"#,
            summary, post_url, post.title
        );
        let note = Activity::note(
            content,
            post_url.clone(),
            Utc::now(),
            vec![activities::PUBLIC_TIMELINE.into()],
            vec![],
        );
        let create = Activity::create(
            settings.activitypub_actor_uri(),
            note,
            vec![activities::PUBLIC_TIMELINE.into()],
            vec![],
        );

        let inserted = query!(
            "INSERT INTO activitypub_outbox(activity_id, activity, source_post, site_id) VALUES($1, $2, $3, $4) RETURNING id",
            post_url,
            Json(create) as _,
            post.id,
			globals.site_id
        )
        .fetch_one(&globals.connection_pool)
        .await?;
        if push {
            for f in &followers {
                query!("INSERT INTO activitypub_outbox_target(activitypub_outbox_id, target) VALUES ($1, $2)", inserted.id, f).execute(&globals.connection_pool).await?;
            }
        }
    }

    Ok(())
}

pub async fn publish_profile_updates(globals: PageGlobals) -> anyhow::Result<cgi::Response> {
    let settings = get_settings_struct(&globals.connection_pool, globals.site_id).await?;
    let follower_rows =
        query!("SELECT actor FROM activitypub_known_actors aka WHERE EXISTS (SELECT 1 FROM activitypub_followers af WHERE af.actor_id = aka.id AND af.site_id=$1)", globals.site_id)
            .fetch_all(&globals.connection_pool)
            .await?;
    let followers: Vec<String> = follower_rows
        .into_iter()
        .map(|r| r.actor.unwrap())
        .collect();
    let activity_url = format!(
        "{}/updates/{}",
        settings.activitypub_base(),
        Uuid::new_v4().hyphenated()
    );
    let update = Activity::Update(Update::new(
        settings.activitypub_actor_uri(),
        settings.activitypub_actor_uri(),
        Activity::Person(Actor::new(settings)),
        vec![activities::PUBLIC_TIMELINE.into()],
        vec![],
    ));
    let inserted = query!(
        "INSERT INTO activitypub_outbox(activity_id, activity, site_id) VALUES($1, $2, $3) RETURNING id",
        activity_url,
        Json(update) as _,
		globals.site_id
    )
    .fetch_one(&globals.connection_pool)
    .await?;
    for f in followers {
        query!(
            "INSERT INTO activitypub_outbox_target(activitypub_outbox_id, target) VALUES ($1, $2)",
            inserted.id,
            f
        )
        .execute(&globals.connection_pool)
        .await?;
    }

    render_redirect("dashboard")
}

pub async fn send(request: &cgi::Request, globals: PageGlobals) -> anyhow::Result<cgi::Response> {
    let id = globals
        .query
        .get("id")
        .ok_or(anyhow!("No id"))?
        .parse::<i32>()?;
    let settings = get_settings_struct(&globals.connection_pool, globals.site_id).await?;
    match request.method() {
        &Method::GET => {
            let common = get_common(&globals, crate::types::AdminMenuPages::Posts).await?;
            let post = query!(
                "SELECT title, url_slug, post_date FROM posts WHERE id=$1 AND site_id=$2",
                id,
                globals.site_id
            )
            .fetch_one(&globals.connection_pool)
            .await?;

            let post_url = blog_post_url(post.url_slug, post.post_date, settings.base_url)?;
            render_html(SendPage {
                message: format!(r#"Check out <a href="{}">{}</a>"#, post_url, post.title),
                common,
            })
        }
        &Method::POST => {
            let body = post_body::<HashMap<String, String>>(&request)?;
            let note_id = format!(
                "{}/notes/{}",
                settings.activitypub_base(),
                Uuid::new_v4().hyphenated()
            );
            let message = body.get("message").unwrap().to_owned();
            let to = body.get("to").unwrap().to_owned();
            let note = Activity::note(
                message,
                note_id.clone(),
                Utc::now(),
                vec![activities::PUBLIC_TIMELINE.into()],
                vec![to.clone()],
            );
            let create = Activity::create(
                settings.activitypub_actor_uri(),
                note,
                vec![activities::PUBLIC_TIMELINE.into()],
                vec![to.clone()],
            );

            let inserted = query!(
                "INSERT INTO activitypub_outbox(activity_id, activity, site_id) VALUES($1, $2, $3) RETURNING id",
                note_id,
                Json(create) as _,
				globals.site_id
            )
            .fetch_one(&globals.connection_pool)
            .await?;

            query!("INSERT INTO activitypub_outbox_target(activitypub_outbox_id, target) VALUES ($1, $2)", inserted.id, to).execute(&globals.connection_pool).await?;

            render_redirect("dashboard")
        }
        _ => bail!("Unknown method"),
    }
}

pub async fn feed(globals: PageGlobals) -> anyhow::Result<cgi::Response> {
    let common = get_common(&globals, crate::types::AdminMenuPages::Fediverse).await?;
    let messages = query_as!(
        FeedMessage,
        r#"
SELECT
	(CASE WHEN a.username IS NOT NULL THEN a.username || '@' || a.server ELSE a.actor END) AS actor,
	message_timestamp AS "timestamp",
	message
FROM activitypub_feed f
INNER JOIN activitypub_known_actors a
ON a.id = f.actor_id
WHERE site_id=$1
ORDER BY message_timestamp DESC
"#,
        globals.site_id
    )
    .fetch_all(&globals.connection_pool)
    .await?;

    render_html(FeedPage { common, messages })
}
