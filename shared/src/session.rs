use std::collections::HashMap;

use anyhow::bail;
use chrono::Utc;
use serde_querystring::{ParseMode, from_str};
use sqlx::PgPool;

pub async fn has_valid_session(connection: &PgPool, request: &cgi::Request) -> anyhow::Result<()> {
    let headers = request.headers();
    if !headers.contains_key(cgi::http::header::COOKIE) {
        bail!("No cookie!") // Until I update the cookie...
    } else {
        let cookie_header = headers[cgi::http::header::COOKIE].to_str();
        match cookie_header {
            Ok(cookie_str) => {
                let cookie_parts: HashMap<String, String> =
                    from_str(cookie_str, ParseMode::UrlEncoded)?;

                match cookie_parts.get("blog_session") {
                    Some(blog_session) => {
                        let saved_session = sqlx::query!(
                            "SELECT expiry FROM session WHERE id=$1::uuid",
                            blog_session as _
                        )
                        .fetch_one(connection)
                        .await?;
                        let now = Utc::now();
                        if saved_session.expiry < now {
                            bail!("Expired")
                        } else {
                            Ok(())
                        }
                    }
                    _ => bail!("No session part in {:?}", cookie_parts),
                }
            }
            _ => bail!("No cookie"),
        }
    }
}
