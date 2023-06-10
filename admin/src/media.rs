use anyhow::anyhow;
use askama::Template;
use bytes::Bytes;
use futures_util::stream::once;
use image::{imageops::FilterType, io::Reader as ImageReader};
use itertools::Itertools;
use lazy_static::lazy_static;
use multer::Multipart;
use shared::{database::connect_db, types::ImageMetadata, utils::render_html};
use sqlx::{query, types::Json};
use std::io::Cursor;
use std::{collections::HashMap, convert::Infallible};

use crate::common::{get_common, Common};

#[derive(Template)]
#[template(path = "media.html")]
struct ManageMedia {
    common: Common,
    media: Vec<MediaItem>,
}

struct MediaItem {
    id: i32,
    file: String,
    metadata: ImageMetadata,
}

lazy_static! {
    static ref ALLOWED_CONTENT_TYPES: Vec<&'static str> =
        vec!["image/jpeg", "image/png", "image/gif"];
    static ref TYPE_EXTENSIONS: HashMap<&'static str, &'static str> = HashMap::from([
        ("image/jpeg", "jpg"),
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

        while let Some(field) = uploaded.next_field().await? {
            let filename = field.file_name().ok_or(anyhow!("No filename!"))?.to_owned();
            let content_type = field
                .content_type()
                .ok_or(anyhow!("No content type!"))?
                .to_owned();

            let type_str = content_type.essence_str();

            if !ALLOWED_CONTENT_TYPES.contains(&type_str) {
                return Err(anyhow!(format!("Content type {} not allowed!", type_str)));
            }
            let bytes = field.bytes().await?;

            let image = ImageReader::new(Cursor::new(bytes))
                .with_guessed_format()?
                .decode()?;

            let ext = TYPE_EXTENSIONS
                .get(type_str)
                .ok_or(anyhow!("No extension found!"))?;

            let result = query!(
                "INSERT INTO media(file_type, file) VALUES('image', $1) RETURNING id",
                filename,
            )
            .fetch_one(&mut conn)
            .await?;

            let disk_name = format!("{}_orig.{}", result.id, ext);
            let thumbnail_name = format!("{}_thumb.{}", result.id, ext);
            let metadata = ImageMetadata {
                width: image.width(),
                height: image.height(),
                content_type: type_str.into(),
                fullsize_name: disk_name.clone(),
                thumbnail_name: thumbnail_name.clone(),
            };

            query!(
                "UPDATE media SET metadata=$1 WHERE id=$2",
                Json(metadata) as _,
                result.id
            )
            .execute(&mut conn)
            .await?;

            image.save(format!("{}{}", media_path, disk_name))?;

            let thumb = if image.width() > 128 || image.height() > 128 {
                image.resize(128, 128, FilterType::Lanczos3)
            } else {
                image
            };

            thumb.save(format!("{}{}", media_path, thumbnail_name))?;
        }
    }

    let media_raw = query!(
        r#"SELECT id, file, metadata AS "metadata: Json<ImageMetadata>" FROM media ORDER BY id asc"#
    )
    .fetch_all(&mut conn)
    .await?;

    let media = media_raw
        .into_iter()
        .map(|m| MediaItem {
            id: m.id,
            file: m.file,
            metadata: m.metadata.unwrap().as_ref().clone(),
        })
        .collect_vec();

    let common = get_common(&mut conn, crate::types::AdminMenuPages::Media).await?;
    render_html(ManageMedia { common, media })
}
