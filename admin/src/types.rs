use core::fmt;

use chrono::NaiveDate;
use serde::Deserialize;
use shared::types::PostStatus;

pub enum AdminMenuPages {
    Dashboard,
    Account,
    Posts,
    NewPost,
    Settings,
    Links,
    Comments,
    Pages,
    Media,
    Fediverse,
    Tags,
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
            AdminMenuPages::Pages => write!(f, "pages"),
            AdminMenuPages::Media => write!(f, "media"),
            AdminMenuPages::Fediverse => write!(f, "fediverse"),
            AdminMenuPages::Tags => write!(f, "tags"),
        }
    }
}

impl PartialEq<&str> for AdminMenuPages {
    fn eq(&self, rhs: &&str) -> bool {
        let str_value = self.to_string();
        return str_value == *rhs;
    }
}

#[derive(Deserialize)]
pub struct PostRequest {
    pub title: String,
    pub body: String,
    pub date: NaiveDate,
    pub status: PostStatus,
    pub slug: String,
    pub song: Option<String>,
    pub mood: Option<String>,
    pub summary: Option<String>,
    pub tags: Option<Vec<i32>>,
}
