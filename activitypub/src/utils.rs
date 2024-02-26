use cgi::http::{header, response};
use shared::settings::{get_settings_struct, SettingNames, Settings};
use sqlx::{query, PgPool};

pub fn jsonld_response<T>(content: &T) -> anyhow::Result<cgi::Response>
where
    T: ?Sized + serde::Serialize,
{
    let body = serde_json::to_vec(content)?;
    let response = response::Builder::new()
        .status(200)
        .header(header::CONTENT_LENGTH, format!("{}", body.len()).as_str())
        .header(
            header::CONTENT_TYPE,
            r#"application/ld+json; profile="https://www.w3.org/ns/activitystreams""#,
        )
        .body(body)?;
    Ok(response)
}

pub async fn settings_for_actor(
    connection: &PgPool,
    hostname: &str,
    actor_name: &str,
) -> anyhow::Result<Settings> {
    let target = query!("SELECT site_id FROM blog_settings b WHERE b.setting_name = $1 AND b.value=$2 AND EXISTS (SELECT 1 FROM blog_settings b2 WHERE b2.setting_name = $3 AND b2.value = $4 AND b.site_id = b2.site_id)",

                        SettingNames::CanonicalHostname.to_string(),
                        hostname,
                        SettingNames::ActorName.to_string(),
                        actor_name
    ).fetch_one(connection).await?;

    get_settings_struct(connection, target.site_id).await
}
