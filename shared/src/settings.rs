use sqlx::{query, PgConnection};

use std::{
    collections::HashMap,
    fmt::{self, Display},
    str::FromStr,
};

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum SettingNames {
    BlogName,
    BaseUrl,
    StaticBaseUrl,
    CommentCGIUrl,
    MediaPath,
    MediaBaseUrl,
    CanonicalHostname,
    FediPublicKeyPem,
    FediPrivateKeyPem,
}
const BLOG_NAME: &str = "blog_name";
const BASE_URL: &str = "base_url";
const STATIC_BASE_URL: &str = "static_base_url";
const COMMENT_CGI_URL: &str = "comment_cgi_url";
const MEDIA_PATH: &str = "media_path";
const MEDIA_BASE_URL: &str = "media_base_url";
const CANONICAL_HOSTNAME: &str = "canonical_hostname";
const FEDI_PUBLIC_KEY_PEM: &str = "fedi_public_key_pem";
const FEDI_PRIVATE_KEY_PEM: &str = "fedi_private_key_pem";

impl Display for SettingNames {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SettingNames::BlogName => write!(f, "{}", BLOG_NAME),
            SettingNames::BaseUrl => write!(f, "{}", BASE_URL),
            SettingNames::StaticBaseUrl => write!(f, "{}", STATIC_BASE_URL),
            SettingNames::CommentCGIUrl => write!(f, "{}", COMMENT_CGI_URL),
            SettingNames::MediaPath => write!(f, "{}", MEDIA_PATH),
            SettingNames::MediaBaseUrl => write!(f, "{}", MEDIA_BASE_URL),
            SettingNames::CanonicalHostname => write!(f, "{}", CANONICAL_HOSTNAME),
            SettingNames::FediPublicKeyPem => write!(f, "{}", FEDI_PUBLIC_KEY_PEM),
            SettingNames::FediPrivateKeyPem => write!(f, "{}", FEDI_PRIVATE_KEY_PEM),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ParseSettingNamesError;

impl FromStr for SettingNames {
    type Err = ParseSettingNamesError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            BLOG_NAME => Ok(SettingNames::BlogName),
            BASE_URL => Ok(SettingNames::BaseUrl),
            STATIC_BASE_URL => Ok(SettingNames::StaticBaseUrl),
            COMMENT_CGI_URL => Ok(SettingNames::CommentCGIUrl),
            MEDIA_PATH => Ok(SettingNames::MediaPath),
            MEDIA_BASE_URL => Ok(SettingNames::MediaBaseUrl),
            CANONICAL_HOSTNAME => Ok(SettingNames::CanonicalHostname),
            FEDI_PUBLIC_KEY_PEM => Ok(SettingNames::FediPublicKeyPem),
            FEDI_PRIVATE_KEY_PEM => Ok(SettingNames::FediPrivateKeyPem),
            _ => Err(ParseSettingNamesError),
        }
    }
}

pub struct Settings {
    pub blog_name: String,
    pub base_url: String,
    pub static_base_url: String,
    pub comment_cgi_url: String,
    pub media_path: String,
    pub media_base_url: String,
    pub canonical_hostname: String,
    pub fedi_public_key_pem: String,
    pub fedi_private_key_pem: String,
}

pub async fn get_settings(
    connection: &mut PgConnection,
) -> anyhow::Result<HashMap<SettingNames, String>> {
    let all_settings = query!("SELECT setting_name, value FROM blog_settings")
        .fetch_all(connection)
        .await?;

    let map = HashMap::from_iter(
        all_settings
            .into_iter()
            .map(|r| (r.setting_name.parse().unwrap(), r.value)),
    );

    Ok(map)
}

pub async fn get_settings_struct(connection: &mut PgConnection) -> anyhow::Result<Settings> {
    let all_settings = get_settings(connection).await?;

    Ok(Settings {
        blog_name: all_settings
            .get(&SettingNames::BlogName)
            .unwrap_or(&"My Blog".into())
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
            .get(&SettingNames::FediPublicKeyPem)
            .unwrap_or(&"Private Key".into())
            .into(),
    })
}
