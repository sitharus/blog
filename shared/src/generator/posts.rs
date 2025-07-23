use anyhow::{Result, anyhow};
use chrono::{Datelike, Month};
use num_traits::FromPrimitive;
use serde::Serialize;
use sqlx::{query, query_as, types::Json};
use tera::Context;
use tokio::{
    fs::{File, create_dir_all},
    io::AsyncWriteExt,
};

use crate::{
    activities::Activity,
    types::{CommonData, HydratedComment, HydratedPost},
};

use super::types::Generator;

#[derive(Serialize)]
struct PostPage<'a> {
    title: &'a String,
    post: &'a HydratedPost,
    common: &'a CommonData,
    comments: Vec<HydratedComment>,
}

pub async fn generate_post_html(generator: &Generator<'_>, post: &HydratedPost) -> Result<String> {
    let comments = if post.id > 0 {
        query_as!(HydratedComment, "SELECT author_name, post_body, created_date FROM comments WHERE post_id=$1 AND status = 'approved' ORDER BY created_date ASC", post.id)
                                   .fetch_all(generator.pool).await?
    } else {
        vec![]
    };

    let post_page = PostPage {
        title: &post.title,
        post,
        common: generator.common,
        comments,
    };

    Ok(generator
        .tera
        .render("post.html", &Context::from_serialize(&post_page)?)?)
}

pub async fn generate_post_page(generator: &Generator<'_>, post: &HydratedPost) -> Result<()> {
    let rendered = generate_post_html(generator, post).await?;
    let post_date = post.post_date.with_timezone(&generator.common.timezone);
    let month_name = Month::from_u32(post_date.month())
        .ok_or(anyhow!("Bad month number"))?
        .name();
    let dir = format!(
        "{}/{}/{}",
        generator.output_path,
        post_date.year(),
        month_name
    );
    let post_path = format!("{}/{}.html", &dir, post.url_slug);
    create_dir_all(&dir).await?;
    let mut file = File::create(post_path).await?;
    file.write_all(rendered.as_bytes()).await?;

    let activitypub = query!(
        r#"SELECT activity AS "activity: Json<Activity>" FROM activitypub_outbox WHERE source_post=$1"#,
        post.id
    )
    .fetch_optional(generator.pool)
    .await?;

    if let Some(row) = activitypub {
        if let Activity::Create(create) = row.activity.as_ref() {
            if let Activity::Note(note) = create.object() {
                let json_path = format!("{}/{}.json", &dir, post.url_slug);
                let mut json_file = File::create(json_path).await?;
                json_file
                    .write_all(serde_json::to_string(note)?.as_bytes())
                    .await?;
            }
        }
    }

    Ok(())
}
