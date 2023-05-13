use sqlx::postgres::PgConnection;
use serde::Deserialize;
use serde_querystring::{from_str, ParseMode};
use uuid::Uuid;
use time::ext::NumericalDuration;

pub struct Session {
    pub id: Uuid,
    pub user_id: i32,
    pub expiry: time::OffsetDateTime,
}

#[derive(Debug)]
pub enum SessionError {
	NotFound,
	Expired
}

#[derive(Deserialize)]
struct Cookie {
    blog_session: String,
}

impl std::fmt::Display for SessionError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			SessionError::NotFound => write!(f, "Not Found"),
			SessionError::Expired => write!(f, "Expired")
		}
	}
}

impl std::error::Error for SessionError {
	fn description(&self) -> &str {
		match self {
			SessionError::Expired => "Session expired",
			SessionError::NotFound => "Invalid session"
		}
	}
}

pub async fn session_id(connection: &mut PgConnection, request: &cgi::Request) -> anyhow::Result<Session, SessionError> {
	let headers = request.headers();
	if !headers.contains_key(http::header::COOKIE) {
		return Err(SessionError::NotFound)
	}
    let cookie_header = headers[http::header::COOKIE].to_str();
    match cookie_header {
        Ok(cookie_str) => {
            let cookie_parts: Result<Cookie, _> = from_str(cookie_str, ParseMode::UrlEncoded);
            match cookie_parts {
                Ok(Cookie { blog_session }) => {
                    let session_id = Uuid::parse_str(&blog_session).unwrap();
                    let saved_session =
                        sqlx::query_as!(Session, "SELECT * FROM session WHERE id=$1", session_id)
                            .fetch_one(connection)
                            .await
                        .unwrap();
					let now = time::OffsetDateTime::now_utc();
					if saved_session.expiry < now {
						Err(SessionError::Expired)
					} else {
						Ok(saved_session)
					}
                }
                _ => Err(SessionError::Expired),
            }
        }
        _ => Err(SessionError::NotFound),
    }
}

pub async fn set_session_and_redirect(connection: &mut PgConnection, user_id: i32, destination: &str) -> anyhow::Result<cgi::Response> {

		let new_session_id = Uuid::new_v4();
		let session_expiry = time::OffsetDateTime::now_utc().checked_add(2.days());
		sqlx::query!("INSERT INTO session(id, user_id, expiry) VALUES($1, $2, $3)", new_session_id, user_id, session_expiry)
			.execute(connection)
			.await?;
        let body: Vec<u8> = "Redirecting".as_bytes().to_vec();
        Ok(http::response::Builder::new()
            .status(302)
            .header(http::header::LOCATION, format!("?action={}", destination))
            .header(http::header::SET_COOKIE, format!("blog_session={}; HttpOnly", new_session_id))
            .header(http::header::CONTENT_TYPE, "text/plain")
            .body(body)
            .unwrap())
}
