use std::collections::HashMap;

use anyhow::bail;
use chrono::Utc;
use serde_querystring::{from_str, ParseMode};
use sqlx::PgConnection;

pub async fn has_valid_session(
    connection: &mut PgConnection,
    request: &cgi::Request,
) -> anyhow::Result<()> {
    let headers = request.headers();
    if !headers.contains_key(http::header::COOKIE) {
        bail!("No");
    } else {
        let cookie_header = headers[http::header::COOKIE].to_str();
        match cookie_header {
            Ok(cookie_str) => {
                let cookie_parts: HashMap<String, String> =
                    from_str(cookie_str, ParseMode::UrlEncoded)?;

                match cookie_parts.get("blog_session") {
                    Some(blog_session) => {
                        let saved_session = sqlx::query!(
                            "SELECT expiry FROM session WHERE id=$1",
                            blog_session as _
                        )
                        .fetch_one(connection)
                        .await?;
                        let now = Utc::now();
                        if saved_session.expiry < now {
                            bail!("No")
                        } else {
                            Ok(())
                        }
                    }
                    _ => bail!("No"),
                }
            }
            _ => bail!("No"),
        }
    }
}
