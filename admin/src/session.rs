use anyhow::anyhow;
use cgi::http;
use chrono::{DateTime, naive::Days, offset::Utc};
use serde::Deserialize;
use serde_querystring::{ParseMode, from_str};
use sqlx::PgPool;
use uuid::Uuid;

pub struct Session {
    #[allow(dead_code)]
    pub id: Uuid,
    #[warn(dead_code)]
    pub user_id: i32,
    pub expiry: DateTime<Utc>,
}

#[derive(Debug)]
pub enum SessionError {
    NotFound,
    Expired,
}

#[derive(Deserialize)]
struct Cookie {
    blog_session: String,
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionError::NotFound => write!(f, "Not Found"),
            SessionError::Expired => write!(f, "Expired"),
        }
    }
}

impl std::error::Error for SessionError {
    fn description(&self) -> &str {
        match self {
            SessionError::Expired => "Session expired",
            SessionError::NotFound => "Invalid session",
        }
    }
}

pub async fn session_id(connection: &PgPool, request: &cgi::Request) -> anyhow::Result<Session> {
    let headers = request.headers();
    if !headers.contains_key(cgi::http::header::COOKIE) {
        return Err(anyhow!(SessionError::NotFound));
    }
    let cookie_header = headers[cgi::http::header::COOKIE].to_str();
    match cookie_header {
        Ok(cookie_str) => {
            let cookie_parts: Result<Cookie, _> = from_str(cookie_str, ParseMode::UrlEncoded);
            match cookie_parts {
                Ok(Cookie { blog_session }) => {
                    let session_id = Uuid::parse_str(&blog_session)?;
                    let saved_session =
                        sqlx::query_as!(Session, "SELECT * FROM session WHERE id=$1", session_id)
                            .fetch_one(connection)
                            .await?;
                    let now = Utc::now();
                    if saved_session.expiry < now {
                        Err(anyhow!(SessionError::Expired))
                    } else {
                        Ok(saved_session)
                    }
                }
                _ => Err(anyhow!(SessionError::Expired)),
            }
        }
        _ => Err(anyhow!(SessionError::NotFound)),
    }
}

pub async fn set_session_and_redirect(
    connection: &PgPool,
    user_id: i32,
    destination: &str,
) -> anyhow::Result<cgi::Response> {
    let new_session_id = Uuid::new_v4();
    let session_expiry = Utc::now().checked_add_days(Days::new(2));
    sqlx::query!(
        "INSERT INTO session(id, user_id, expiry) VALUES($1, $2, $3)",
        new_session_id,
        user_id,
        session_expiry
    )
    .execute(connection)
    .await?;
    let body: Vec<u8> = "Redirecting".as_bytes().to_vec();
    let result = http::response::Builder::new()
        .status(302)
        .header(http::header::LOCATION, format!("?action={}", destination))
        .header(
            http::header::SET_COOKIE,
            format!("blog_session={}; HttpOnly; Path=/", new_session_id),
        )
        .header(http::header::CONTENT_TYPE, "text/plain")
        .body(body)?;

    Ok(result)
}
