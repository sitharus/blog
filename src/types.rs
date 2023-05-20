use core::fmt;

#[derive(serde::Deserialize, sqlx::Type, std::fmt::Debug, PartialEq)]
#[sqlx(type_name = "post_status")] // only for PostgreSQL to match a type definition
#[sqlx(rename_all = "lowercase")]
pub enum PostStatus {
    Draft,
    Published,
}

pub struct Post {
    pub id: i32,
    pub author_id: i32,
    pub post_date: time::Date,
    pub created_date: time::OffsetDateTime,
    pub updated_date: time::OffsetDateTime,
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
        }
    }
}

impl PartialEq<&str> for AdminMenuPages {
    fn eq(&self, rhs: &&str) -> bool {
        let str_value = self.to_string();
        return str_value == *rhs;
    }
}
