use std::{collections::HashMap, convert::Infallible, fs::File, io::Write};

use anyhow::anyhow;
use askama::Template;
use bytes::Bytes;
use futures_util::stream::once;
use lazy_static::lazy_static;
use multer::Multipart;
use shared::{database::connect_db, utils::render_html};
use sqlx::query;

use crate::common::{get_common, Common};

#[derive(Template)]
#[template(path = "media.html")]
struct ManageMedia {
    common: Common,
}

lazy_static! {
    static ref ALLOWED_CONTENT_TYPES: Vec<&'static str> =
        vec!["image/jpg", "image/png", "image/gif"];
    static ref TYPE_EXTENSIONS: HashMap<&'static str, &'static str> = HashMap::from([
        ("image/jpg", "jpg"),
        ("image/png", "png"),
        ("image/gif", "gif"),
    ]);
}

pub async fn manage_media(request: &cgi::Request) -> anyhow::Result<cgi::Response> {
    let mut conn = connect_db().await?;
    if request.method() == "POST" {
        let mut media_path =
            query!("SELECT value FROM blog_settings WHERE setting_name='media_path'")
                .fetch_one(&mut conn)
                .await?
                .value;

        if !media_path.ends_with("/") {
            media_path = format!("{}/", media_path);
        }

        let content_type = request
            .headers()
            .get("x-cgi-content-type")
            .ok_or(anyhow!("No content type! Must be multipart.form_data!"))
            .and_then(|x| x.to_str().map_err(|e| anyhow!(e)))?;
        let boundary = multer::parse_boundary(content_type)?;

        let slice = request.body().to_owned();
        let stream = once(async move { Result::<Bytes, Infallible>::Ok(Bytes::from(slice)) });

        let mut uploaded = Multipart::new(stream, boundary);

        while let Some(mut field) = uploaded.next_field().await? {
            let filename = field.file_name().ok_or(anyhow!("No filename!"))?;
            let content_type = field.content_type().ok_or(anyhow!("No content type!"))?;

            let type_str = content_type.essence_str();

            if !ALLOWED_CONTENT_TYPES.contains(&type_str) {
                return Err(anyhow!("Content type not allowed!"));
            }

            let ext = TYPE_EXTENSIONS
                .get(type_str)
                .ok_or(anyhow!("No extension found!"))?;

            let result = query!(
                "INSERT INTO media(file_type, file, metadata) VALUES('image', $1, NULL) RETURNING id",
                filename
            ).fetch_one(&mut conn)
            .await?;
            let disk_name = format!("{}{}_orig.{}", media_path, result.id, ext);

            let mut file = File::create(disk_name)?;

            while let Some(chunk) = field.chunk().await? {
                file.write(&chunk)?;
            }
        }
    }

    let common = get_common(&mut conn, crate::types::AdminMenuPages::Media).await?;
    render_html(ManageMedia { common })
}
