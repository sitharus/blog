use std::collections::HashMap;

use anyhow::anyhow;
use shared::{
    activities::{self, Activity},
    database::connect_db,
    settings::get_settings_struct,
    utils::{blog_post_url, render_redirect},
};
use sqlx::{query, types::Json};

pub async fn publish_posts(
    _request: &cgi::Request,
    query: HashMap<String, String>,
) -> anyhow::Result<cgi::Response> {
    let push = match query.get("push").map(|f| f.as_str()) {
        Some("true") => true,
        _ => false,
    };
    let mut connection = connect_db().await?;
    let settings = get_settings_struct(&mut connection).await?;
    let follower_rows =
        query!("SELECT actor FROM activitypub_known_actors WHERE is_following = true")
            .fetch_all(&mut connection)
            .await?;
    let followers: Vec<String> = follower_rows
        .into_iter()
        .map(|r| r.actor.unwrap())
        .collect();

    let to_post = query!("SELECT title, url_slug, post_date FROM posts p WHERE NOT EXISTS (SELECT 1 FROM activitypub_outbox o WHERE o.source_post = p.id) AND p.state = 'published'")
        .fetch_all(&mut connection)
        .await?;

    for post in to_post {
        let post_url = blog_post_url(post.url_slug, post.post_date, settings.base_url.clone())?;
        let post_date = post
            .post_date
            .and_hms_opt(0, 0, 0)
            .ok_or(anyhow!("Failed to make time"))?
            .and_utc();

        let content = format!(r#"New post! <a href="{}">{}</a>"#, post_url, post.title);
        let note = Activity::note(
            content,
            post_url.clone(),
            post_date,
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
            "INSERT INTO activitypub_outbox(activity_id, activity) VALUES($1, $2) RETURNING id",
            post_url,
            Json(create) as _
        )
        .fetch_one(&mut connection)
        .await?;
        if push {
            for f in &followers {
                query!("INSERT INTO activitypub_outbox_target(activitypub_outbox_id, target) VALUES ($1, $2)", inserted.id, f).execute(&mut connection).await?;
            }
        }
    }

    render_redirect("dashboard")
}
