use itertools::Itertools;
use serde::Serialize;
use sqlx::query;
use std::vec::IntoIter;
use tera::Context;
use tokio::{
    fs::{create_dir_all, File},
    io::AsyncWriteExt,
};

use crate::types::{CommonData, HydratedPost};

use super::types::Generator;

#[derive(Serialize)]
struct IndexPage<'a> {
    title: &'a str,
    common: &'a CommonData,
    posts: Vec<&'a HydratedPost>,
    page: i32,
    total_pages: i32,
}

pub async fn generate_index_pages<'a>(
    posts: IntoIter<&HydratedPost>,
    generator: &Generator<'a>,
) -> anyhow::Result<()> {
    generate_pages(posts, generator, generator.output_path).await
}

pub fn index_content<'a>(
    posts: Vec<&HydratedPost>,
    generator: &Generator<'a>,
    page_number: i32,
    total_pages: i32,
) -> anyhow::Result<String> {
    let page = IndexPage {
        title: &generator.common.blog_name,
        posts,
        common: &generator.common,
        page: page_number,
        total_pages,
    };

    let result = generator
        .tera
        .render("index.html", &Context::from_serialize(page)?)?;
    Ok(result)
}

async fn generate_pages<'a>(
    posts: IntoIter<&HydratedPost>,
    generator: &Generator<'a>,
    output_path: &'a str,
) -> anyhow::Result<()> {
    let total_pages = (posts.len() as f64 / 10.0).ceil() as i32;
    for (pos, chunk) in posts.chunks(10).into_iter().enumerate() {
        let path = if pos == 0 {
            String::from("index.html")
        } else {
            format!("index{}.html", pos + 1)
        };
        let posts = chunk.collect();

        let rendered = index_content(posts, generator, pos as i32 + 1, total_pages)?;
        let mut file = File::create(format!("{}/{}", output_path, path)).await?;
        file.write_all(rendered.as_bytes()).await?;
    }
    Ok(())
}

pub async fn generate_tag_indexes<'a>(
    posts: &Vec<HydratedPost>,
    generator: &Generator<'a>,
) -> anyhow::Result<()> {
    let all_tags = query!("SELECT name FROM tags WHERE site_id=$1", generator.site_id)
        .fetch_all(generator.pool)
        .await?;
    for tag_row in all_tags {
        let tag = tag_row.name;

        let tag_posts = posts
            .iter()
            .filter(|p| p.tags.clone().map(|t| t.contains(&tag)).unwrap_or(false))
            .sorted_by(|a, b| Ord::cmp(&b.post_date, &a.post_date));

        let tag_output_path = format!("{}/tags/{}/", generator.output_path, tag.to_lowercase());
        create_dir_all(&tag_output_path).await?;
        generate_pages(tag_posts.into_iter(), generator, &tag_output_path).await?;
    }
    Ok(())
}
