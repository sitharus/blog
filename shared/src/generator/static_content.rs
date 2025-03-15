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
