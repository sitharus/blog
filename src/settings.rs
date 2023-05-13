use std::collections::HashMap;

use crate::types::AdminMenuPages;
use askama::Template;
use cgi;
use serde_querystring::from_bytes;
use sqlx::query;

use super::database;
use super::session;

#[derive(Template)]
#[template(path = "settings.html")]
struct Settings {
    selected_menu_item: AdminMenuPages,
    blog_name: String,
    base_url: String,
}

pub async fn render(request: &cgi::Request) -> anyhow::Result<cgi::Response> {
    let mut connection = database::connect_db().await?;
    session::session_id(&mut connection, &request).await?;

    if request.method() == "POST" {
        let post_items = request.body();
        let content: HashMap<String, String> =
            from_bytes(post_items, serde_querystring::ParseMode::UrlEncoded)?;

        for setting in ["blog_name", "base_url"] {
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

    let all_settings = query!("SELECT setting_name, value FROM blog_settings")
        .fetch_all(&mut connection)
        .await?;

    let mapped = all_settings.into_iter().map(|r| (r.setting_name, r.value));
    let settings_lookup: HashMap<String, String> = HashMap::from_iter(mapped);

    let blog_name = settings_lookup
        .get("blog_name")
        .unwrap_or(&String::from(""))
        .to_owned();
    let base_url = settings_lookup
        .get("base_url")
        .unwrap_or(&String::from(""))
        .to_owned();
    let page = Settings {
        selected_menu_item: AdminMenuPages::Settings,
        blog_name,
        base_url,
    };
    Ok(cgi::html_response(200, page.render().unwrap()))
}
