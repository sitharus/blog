use crate::types::CommonData;

use super::types::Generator;
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::query;
use tera::Context;
use tokio::{fs::File, io::AsyncWriteExt};

#[derive(Serialize)]
struct Page<'a> {
    common: &'a CommonData,
    title: String,
    body: String,
    last_updated: DateTime<Utc>,
}

pub async fn generate_pages<'a>(generator: &Generator<'a>) -> anyhow::Result<()> {
    let pages = query!(
        "SELECT title, url_slug, body, date_updated FROM pages WHERE site_id=$1",
        generator.site_id
    )
    .fetch_all(generator.pool)
    .await?;

    for page in pages {
        let page_context = Page {
            common: generator.common,
            title: page.title,
            body: page.body,
            last_updated: page.date_updated,
        };
        let mut file =
            File::create(format!("{}/{}.html", generator.output_path, page.url_slug)).await?;

        let rendered = generator
            .tera
            .render("page.html", &Context::from_serialize(page_context)?)?;
        file.write_all(rendered.as_bytes()).await?;
    }
    Ok(())
}

pub async fn generate_single_page<'a>(
    id: i32,
    generator: &Generator<'a>,
) -> anyhow::Result<String> {
    let page = query!(
        "SELECT title, url_slug, body, date_updated FROM pages WHERE site_id=$1 and id = $2",
        generator.site_id,
        id
    )
    .fetch_one(generator.pool)
    .await?;

    let page_context = Page {
        common: generator.common,
        title: page.title,
        body: page.body,
        last_updated: page.date_updated,
    };
    let result = generator
        .tera
        .render("page.html", &Context::from_serialize(page_context)?)?;

    Ok(result)
}
