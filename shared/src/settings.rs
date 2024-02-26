use chrono::{DateTime, Utc};
use sqlx::{query, PgPool};

use std::{
    collections::HashMap,
    fmt::{self, Display},
    str::FromStr,
};

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum SettingNames {
    BlogName,
    ActorName,
    BaseUrl,
    StaticBaseUrl,
    CommentCGIUrl,
    MediaPath,
    MediaBaseUrl,
    CanonicalHostname,
    FediPublicKeyPem,
    FediPrivateKeyPem,
    Timezone,
    FediAvatar,
    FediHeader,
    ProfileLastUpdated,
}
const BLOG_NAME: &str = "blog_name";
const ACTOR_NAME: &str = "actor_name";
const BASE_URL: &str = "base_url";
const STATIC_BASE_URL: &str = "static_base_url";
const COMMENT_CGI_URL: &str = "comment_cgi_url";
const MEDIA_PATH: &str = "media_path";
const MEDIA_BASE_URL: &str = "media_base_url";
const CANONICAL_HOSTNAME: &str = "canonical_hostname";
const FEDI_PUBLIC_KEY_PEM: &str = "fedi_public_key_pem";
const FEDI_PRIVATE_KEY_PEM: &str = "fedi_private_key_pem";
const FEDI_AVATAR: &str = "fedi_avatar";
const FEDI_HEADER: &str = "fedi_header";
const TIMEZONE: &str = "timezone";
const PROFILE_LAST_UPDATED: &str = "profile_last_updated";

impl Display for SettingNames {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            SettingNames::BlogName => BLOG_NAME,
            SettingNames::ActorName => ACTOR_NAME,
            SettingNames::BaseUrl => BASE_URL,
            SettingNames::StaticBaseUrl => STATIC_BASE_URL,
            SettingNames::CommentCGIUrl => COMMENT_CGI_URL,
            SettingNames::MediaPath => MEDIA_PATH,
            SettingNames::MediaBaseUrl => MEDIA_BASE_URL,
            SettingNames::CanonicalHostname => CANONICAL_HOSTNAME,
            SettingNames::FediPublicKeyPem => FEDI_PUBLIC_KEY_PEM,
            SettingNames::FediPrivateKeyPem => FEDI_PRIVATE_KEY_PEM,
            SettingNames::Timezone => TIMEZONE,
            SettingNames::FediAvatar => FEDI_AVATAR,
            SettingNames::FediHeader => FEDI_HEADER,
            SettingNames::ProfileLastUpdated => PROFILE_LAST_UPDATED,
        };
        write!(f, "{}", name)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ParseSettingNamesError;

impl FromStr for SettingNames {
    type Err = ParseSettingNamesError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            BLOG_NAME => Ok(SettingNames::BlogName),
            ACTOR_NAME => Ok(SettingNames::ActorName),
            BASE_URL => Ok(SettingNames::BaseUrl),
            STATIC_BASE_URL => Ok(SettingNames::StaticBaseUrl),
            COMMENT_CGI_URL => Ok(SettingNames::CommentCGIUrl),
            MEDIA_PATH => Ok(SettingNames::MediaPath),
            MEDIA_BASE_URL => Ok(SettingNames::MediaBaseUrl),
            CANONICAL_HOSTNAME => Ok(SettingNames::CanonicalHostname),
            FEDI_PUBLIC_KEY_PEM => Ok(SettingNames::FediPublicKeyPem),
            FEDI_PRIVATE_KEY_PEM => Ok(SettingNames::FediPrivateKeyPem),
            TIMEZONE => Ok(SettingNames::Timezone),
            FEDI_AVATAR => Ok(SettingNames::FediAvatar),
            FEDI_HEADER => Ok(SettingNames::FediHeader),
            PROFILE_LAST_UPDATED => Ok(SettingNames::ProfileLastUpdated),
            _ => Err(ParseSettingNamesError),
        }
    }
}

pub struct Settings {
    pub site_id: i32,
    pub blog_name: String,
    pub base_url: String,
    pub static_base_url: String,
    pub comment_cgi_url: String,
    pub media_path: String,
    pub media_base_url: String,
    pub canonical_hostname: String,
    pub fedi_public_key_pem: String,
    pub fedi_private_key_pem: String,
    pub actor_name: String,
    pub fedi_header: Option<String>,
    pub fedi_avatar: Option<String>,
    pub timezone: chrono_tz::Tz,
    pub profile_last_updated: chrono::DateTime<Utc>,
}

impl Settings {
    pub fn activitypub_base(&self) -> String {
        format!(
            "https://{}/activitypub/{}/",
            self.canonical_hostname, self.actor_name
        )
    }

    pub fn activitypub_actor_uri(&self) -> String {
        format!("{}actor", self.activitypub_base())
    }

    pub fn activitypub_key_id(&self) -> String {
        format!("{}#main-key", self.activitypub_actor_uri())
    }
}

pub async fn get_settings(
    connection: &PgPool,
    site_id: i32,
) -> anyhow::Result<HashMap<SettingNames, String>> {
    let all_settings = query!(
        "SELECT setting_name, value FROM blog_settings WHERE site_id=$1",
        site_id
    )
    .fetch_all(connection)
    .await?;

    let map = HashMap::from_iter(
        all_settings
            .into_iter()
            .map(|r| (r.setting_name.parse().unwrap(), r.value)),
    );

    Ok(map)
}

pub async fn get_settings_struct(connection: &PgPool, site_id: i32) -> anyhow::Result<Settings> {
    let all_settings = get_settings(connection, site_id).await?;

    Ok(Settings {
        site_id,
        blog_name: all_settings
            .get(&SettingNames::BlogName)
            .unwrap_or(&"My Blog".into())
            .into(),
        actor_name: all_settings
            .get(&SettingNames::ActorName)
            .unwrap_or(&"blog".into())
            .into(),
        base_url: all_settings
            .get(&SettingNames::BaseUrl)
            .unwrap_or(&"https://blog.example.com".into())
            .into(),
        static_base_url: all_settings
            .get(&SettingNames::StaticBaseUrl)
            .unwrap_or(&"https://blog.example.com/static".into())
            .into(),
        comment_cgi_url: all_settings
            .get(&SettingNames::CommentCGIUrl)
            .unwrap_or(&"/cgi-bin/comment.cgi".into())
            .into(),
        media_path: all_settings
            .get(&SettingNames::MediaPath)
            .unwrap_or(&"/var/www/blog/media".into())
            .into(),
        media_base_url: all_settings
            .get(&SettingNames::MediaBaseUrl)
            .unwrap_or(&"/media/".into())
            .into(),
        canonical_hostname: all_settings
            .get(&SettingNames::CanonicalHostname)
            .unwrap_or(&"blog.example.com".into())
            .into(),
        fedi_public_key_pem: all_settings
            .get(&SettingNames::FediPublicKeyPem)
            .unwrap_or(&"Public Key".into())
            .into(),
        fedi_private_key_pem: all_settings
            .get(&SettingNames::FediPrivateKeyPem)
            .unwrap_or(&"Private Key".into())
            .into(),
        timezone: all_settings
            .get(&SettingNames::Timezone)
            .and_then(|x| x.parse::<chrono_tz::Tz>().ok())
            .unwrap_or(chrono_tz::UTC),
        fedi_avatar: all_settings.get(&SettingNames::FediAvatar).cloned(),
        fedi_header: all_settings.get(&SettingNames::FediHeader).cloned(),
        profile_last_updated: all_settings
            .get(&SettingNames::ProfileLastUpdated)
            .and_then(|a| {
                DateTime::parse_from_rfc3339(a)
                    .map(|d| d.with_timezone(&Utc))
                    .ok()
            })
            .unwrap_or(Utc::now()),
    })
}
