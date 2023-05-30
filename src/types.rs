use chrono::{offset::Utc, DateTime};
use core::fmt;

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

pub enum AdminMenuPages {
    Dashboard,
    Account,
    Posts,
    NewPost,
    Settings,
    Links,
    Comments,
}
impl fmt::Display for AdminMenuPages {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AdminMenuPages::Dashboard => write!(f, "dashboard"),
            AdminMenuPages::Account => write!(f, "account"),
            AdminMenuPages::Posts => write!(f, "posts"),
            AdminMenuPages::NewPost => write!(f, "newpost"),
            AdminMenuPages::Settings => write!(f, "settings"),
            AdminMenuPages::Links => write!(f, "links"),
            AdminMenuPages::Comments => write!(f, "comments"),
        }
    }
}

impl PartialEq<&str> for AdminMenuPages {
    fn eq(&self, rhs: &&str) -> bool {
        let str_value = self.to_string();
        return str_value == *rhs;
    }
}
