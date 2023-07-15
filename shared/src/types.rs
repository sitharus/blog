use chrono::{DateTime, NaiveDate, Utc};
use std::{collections::HashMap, fmt};

pub struct HydratedPost {
    pub id: i32,
    pub post_date: NaiveDate,
    pub url_slug: String,
    pub title: String,
    pub body: String,
    pub author_name: Option<String>,
    pub comment_count: Option<i64>,
    pub song: Option<String>,
    pub mood: Option<String>,
}

pub struct HydratedComment {
    pub author_name: String,
    pub created_date: DateTime<Utc>,
    pub post_body: String,
}

pub struct Link {
    pub title: String,
    pub destination: String,
}

pub struct PageLink {
    pub title: String,
    pub url_slug: String,
}

pub struct CommonData {
    pub base_url: String,
    pub static_base_url: String,
    pub media_base_url: String,
    pub comment_cgi_url: String,
    pub blog_name: String,
    pub archive_years: Vec<i32>,
    pub links: Vec<Link>,
    pub page_links: Vec<PageLink>,
    pub media: HashMap<i32, Media>,
    pub timezone: chrono_tz::Tz,
}

pub struct Media {
    pub id: i32,
    pub file: String,
    pub metadata: ImageMetadata,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct ImageMetadata {
    pub width: u32,
    pub height: u32,
    pub content_type: String,
    pub fullsize_name: String,
    pub thumbnail_name: String,
}

#[derive(serde::Deserialize, sqlx::Type, fmt::Debug, PartialEq, Clone)]
#[sqlx(type_name = "post_status")] // only for PostgreSQL to match a type definition
#[sqlx(rename_all = "lowercase")]
pub enum PostStatus {
    Draft,
    Preview,
    Published,
}

impl fmt::Display for PostStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(serde::Deserialize, sqlx::Type, fmt::Debug, PartialEq, Clone)]
#[sqlx(type_name = "comment_status")] // only for PostgreSQL to match a type definition
#[sqlx(rename_all = "lowercase")]
pub enum CommentStatus {
    Pending,
    Approved,
    Spam,
}

pub struct Post {
    pub id: i32,
    pub author_id: i32,
    pub post_date: chrono::NaiveDate,
    pub created_date: DateTime<Utc>,
    pub updated_date: DateTime<Utc>,
    pub state: PostStatus,
    pub url_slug: String,
    pub title: String,
    pub body: String,
}
