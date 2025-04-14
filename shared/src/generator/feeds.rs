use crate::types::{CommonData, HydratedPost};
use anyhow::anyhow;
use chrono::{DateTime, Utc};
use serde::Serialize;
use tera::Context;
use tokio::{fs::File, io::AsyncWriteExt};

use super::types::Generator;

#[derive(Serialize)]
struct Feed<'a> {
    common: &'a CommonData,
    posts: &'a [HydratedPost],
    date: DateTime<Utc>,
}

pub async fn generate_rss_feed(
    posts: &[HydratedPost],
    generator: &Generator<'_>,
) -> anyhow::Result<()> {
    let max = ::std::cmp::min(posts.len(), 10);
    let posts_in_feed = posts
        .get(0..max)
        .ok_or(anyhow!("Failed to get posts for feed"))?;

    let feed = Feed {
        common: generator.common,
        posts: posts_in_feed,
        date: Utc::now(),
    };
    let mut file = File::create(format!("{}/feed.rss", generator.output_path)).await?;

    let rendered = generator
        .tera
        .render("rss.xml", &Context::from_serialize(feed)?)?;
    file.write_all(rendered.as_bytes()).await?;

    Ok(())
}

pub async fn generate_atom_feed(
    posts: &[HydratedPost],
    generator: &Generator<'_>,
) -> anyhow::Result<()> {
    let max = ::std::cmp::min(posts.len(), 10);
    let posts_in_feed = posts
        .get(0..max)
        .ok_or(anyhow!("Failed to get posts for feed"))?;

    let feed = Feed {
        common: generator.common,
        posts: posts_in_feed,
        date: Utc::now(),
    };

    let mut file = File::create(format!("{}/feed.atom", generator.output_path)).await?;

    let rendered = generator
        .tera
        .render("atom.xml", &Context::from_serialize(feed)?)?;
    file.write_all(rendered.as_bytes()).await?;

    Ok(())
}
