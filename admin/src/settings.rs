use std::collections::HashMap;

use crate::{
    common::{get_common, Common},
    types::AdminMenuPages,
};
use askama::Template;
use cgi;
use shared::{
    database,
    settings::{get_settings_struct, Settings as SettingsStruct},
    utils::post_body,
};
use sqlx::query;

#[derive(Template)]
#[template(path = "settings.html")]
struct Settings {
    common: Common,
    settings: SettingsStruct,
}

pub async fn render(request: &cgi::Request) -> anyhow::Result<cgi::Response> {
    let mut connection = database::connect_db().await?;

    if request.method() == "POST" {
        let content: HashMap<String, String> = post_body(request)?;

        for setting in [
            "blog_name",
            "base_url",
            "comment_cgi_url",
            "static_base_url",
            "media_path",
            "media_base_url",
            "canonical_hostname",
            "fedi_public_key_pem",
            "fedi_private_key_pem",
        ] {
            if let Some(value) = content.get(setting) {
                query!(
                    "INSERT INTO blog_settings VALUES($1, $2) ON CONFLICT (setting_name) DO UPDATE SET value = EXCLUDED.value",
                    setting,
                    value,
                )
                .execute(&mut connection)
                .await?;
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
