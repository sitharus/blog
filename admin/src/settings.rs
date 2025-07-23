use std::io::Cursor;

use crate::{
    common::{Common, get_common},
    filters,
    types::{AdminMenuPages, PageGlobals},
};
use anyhow::anyhow;
use askama::Template;
use bytes::Bytes;
use chrono::Utc;
use futures_util::stream::once;
use multer::Multipart;
use shared::{
    errors::BlogError,
    settings::{SettingNames, Settings as SettingsStruct, get_settings_struct},
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
const STRING_FIELDS: [&str; 14] = [
    "blog_name",
    "actor_name",
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
    "bsky_username",
    "bsky_password",
];

const FILE_FIELDS: [&str; 2] = ["fedi_avatar", "fedi_header"];
const IMAGE_TYPES: [&str; 2] = ["image/jpeg", "image/png"];

pub async fn render(request: &cgi::Request, globals: PageGlobals) -> anyhow::Result<cgi::Response> {
    if request.method() == "POST" {
        let content_type = request
            .headers()
            .get("x-cgi-content-type")
            .ok_or(anyhow!("No content type! Must be multipart.form_data!"))
            .and_then(|x| x.to_str().map_err(|e| anyhow!(e)))?;
        let boundary = multer::parse_boundary(content_type)?;

        let slice = request.body().to_owned();
        let stream = once(async move { Result::<Bytes, Infallible>::Ok(Bytes::from(slice)) });

        let mut editions_enabled = false;
        let mut uploaded = Multipart::new(stream, boundary);
        while let Some(field) = uploaded.next_field().await? {
            let n = field.name().ok_or(anyhow!("No field name!"))?.to_owned();
            if STRING_FIELDS.contains(&n.as_str()) {
                let content = field.text().await?.clone();
                query!(
                        "INSERT INTO blog_settings VALUES($1, $2, $3) ON CONFLICT (setting_name, site_id) DO UPDATE SET value = EXCLUDED.value",
                        n,
                    content,
					globals.site_id
                    )
                    .execute(&globals.connection_pool)
                    .await?;
            } else if n.as_str() == "editions" {
                editions_enabled = true;
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
                        query!("SELECT value FROM blog_settings WHERE setting_name='media_path' AND site_id=$1", globals.site_id)
                            .fetch_one(&globals.connection_pool)
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

                    query!("INSERT INTO blog_settings VALUES($1, $2, $3) ON CONFLICT (setting_name, site_id) DO UPDATE SET value = EXCLUDED.value", n, filename, globals.site_id)
                        .execute(&globals.connection_pool)
                        .await?;
                }
            }
        }

        query!(
            "INSERT INTO blog_settings VALUES($1, $2, $3) ON CONFLICT (setting_name, site_id) DO UPDATE SET value = EXCLUDED.value",
            SettingNames::ProfileLastUpdated.to_string(),
            Utc::now().to_rfc3339(),
				globals.site_id
        )
        .execute(&globals.connection_pool)
        .await?;

        query!(
            "UPDATE sites SET editions_enabled=$1 WHERE id=$2",
            editions_enabled,
            globals.site_id
        )
        .execute(&globals.connection_pool)
        .await?;
    }

    let common = get_common(&globals, AdminMenuPages::Settings).await?;
    let page = Settings {
        common,
        settings: get_settings_struct(&globals.connection_pool, globals.site_id).await?,
    };
    Ok(cgi::html_response(200, page.render().unwrap()))
}
