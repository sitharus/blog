use tera::Context;
use tokio::{fs::File, io::AsyncWriteExt};

use super::types::Generator;

pub async fn generate_static<'a>(
    generator: &Generator<'a>,
    output_path: &'a str,
) -> anyhow::Result<()> {
    let rendered = generator.tera.render("blog.css", &Context::new())?;
    let mut file = File::create(format!("{}/blog.css", output_path)).await?;
    file.write_all(rendered.as_bytes()).await?;
    Ok(())
}

pub struct StaticContent {
    pub content: String,
    pub content_type: String,
}

pub async fn get_static_content(
    generator: &Generator<'_>,
    content: &str,
) -> anyhow::Result<StaticContent> {
    match content {
        "blog.css" => Ok(StaticContent {
            content: generator.tera.render("blog.css", &Context::new())?,
            content_type: "text/css".into(),
        }),
        _ => anyhow::bail!("Unknown static content!"),
    }
}
