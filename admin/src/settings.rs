use std::io::Cursor;

use crate::{
    common::{get_common, Common},
    types::AdminMenuPages,
};
use anyhow::anyhow;
use askama::Template;
use bytes::Bytes;
use cgi;
use futures_util::stream::once;
use multer::Multipart;
use shared::{
    database,
    errors::BlogError,
    settings::{get_settings_struct, Settings as SettingsStruct},
};
use sqlx::query;
use std::convert::Infallible;
use tokio::{fs::File, io::AsyncWriteExt};

#[derive(Template)]
#[template(path = "settings.html")]
struct Settings {
    common: Common,
    settings: SettingsStruct,
}
const STRING_FIELDS: [&'static str; 11] = [
    "blog_name",
    "base_url",
    "timezone",
    "comment_cgi_url",
    "static_base_url",
    "media_path",
    "media_base_url",
    "canonical_hostname",
    "fedi_public_key_pem",
    "fedi_private_key_pem",
    "timezone",
];

const FILE_FIELDS: [&'static str; 2] = ["fedi_avatar", "fedi_header"];
const IMAGE_TYPES: [&'static str; 2] = ["image/jpeg", "image/png"];

pub async fn render(request: &cgi::Request) -> anyhow::Result<cgi::Response> {
    let mut connection = database::connect_db().await?;

    if request.method() == "POST" {
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
            let n = field.name().ok_or(anyhow!("No field name!"))?.to_owned();
            if STRING_FIELDS.contains(&n.as_str()) {
                let content = field.text().await?.clone();
                query!(
                        "INSERT INTO blog_settings VALUES($1, $2) ON CONFLICT (setting_name) DO UPDATE SET value = EXCLUDED.value",
                        n,
                        content,
                    )
                    .execute(&mut connection)
                            .await?;
            } else if FILE_FIELDS.contains(&n.as_str()) {
                let content_type = field
                    .content_type()
                    .ok_or(anyhow!(BlogError::Input {
                        message: "No content type!".into(),
                        field: n.clone()
                    }))?
                    .to_owned();

                let type_str = content_type.essence_str();
                if IMAGE_TYPES.contains(&type_str) {
                    let media_path_setting =
                        query!("SELECT value FROM blog_settings WHERE setting_name='media_path'")
                            .fetch_one(&mut connection)
                            .await?;

                    let ext = if content_type == "image/png" {
                        "png"
                    } else {
                        "jpg"
                    };
                    let name = if n == "fedi_avatar" {
                        "avatar"
                    } else {
                        "banner"
                    };
                    let filename = format!("{}.{}", name, ext);
                    let media_path = format!("{}/{}", media_path_setting.value, filename);
                    let mut file = File::create(media_path).await?;
                    let bytes = field.bytes().await?;
                    let mut cursor = Cursor::new(bytes);
                    file.write_all_buf(&mut cursor).await?;

                    query!("INSERT INTO blog_settings VALUES($1, $2) ON CONFLICT (setting_name) DO UPDATE SET value = EXCLUDED.value", n, filename)
                        .execute(&mut connection)
                        .await?;
                }
            }
        }
    }

    let common = get_common(&mut connection, AdminMenuPages::Settings).await?;
    let page = Settings {
        common,
        settings: get_settings_struct(&mut connection).await?,
    };
    Ok(cgi::html_response(200, page.render().unwrap()))
}
