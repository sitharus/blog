use std::collections::HashMap;

use crate::{
    common::{get_common, Common},
    types::AdminMenuPages,
};
use askama::Template;
use cgi;
use shared::{database, utils::post_body};
use sqlx::query;

#[derive(Template)]
#[template(path = "settings.html")]
struct Settings {
    common: Common,
    blog_name: String,
    base_url: String,
    static_base_url: String,
    comment_cgi_url: String,
    media_path: String,
    media_base_url: String,
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
    let static_base_url = settings_lookup
        .get("static_base_url")
        .unwrap_or(&String::from("/"))
        .to_owned();
    let comment_cgi_url = settings_lookup
        .get("comment_cgi_url")
        .unwrap_or(&String::from("/cgi-bin/blog.cgi"))
        .to_owned();
    let media_path = settings_lookup
        .get("media_path")
        .unwrap_or(&String::from(""))
        .to_owned();
    let media_base_url = settings_lookup
        .get("media_base_url")
        .unwrap_or(&String::from(""))
        .to_owned();

    let common = get_common(&mut connection, AdminMenuPages::Settings).await?;
    let page = Settings {
        common,
        blog_name,
        base_url,
        static_base_url,
        comment_cgi_url,
        media_path,
        media_base_url,
    };
    Ok(cgi::html_response(200, page.render().unwrap()))
}
