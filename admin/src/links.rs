use anyhow::anyhow;
use askama::Template;
use serde_querystring::from_bytes;
use sqlx::{query, query_as};
use std::collections::HashMap;

use crate::{
    common::{get_common, Common},
    types::{AdminMenuPages, PageGlobals},
};

use shared::utils::{render_html, BlogUtils};
#[derive(Template)]
#[template(path = "links.html")]
struct Links {
    common: Common,
    links: Vec<Link>,
}

struct Link {
    id: i32,
    title: String,
    destination: String,
    position: i32,
}

pub async fn render(request: &cgi::Request, globals: PageGlobals) -> anyhow::Result<cgi::Response> {
    if request.method() == "POST" {
        let data: HashMap<String, String> =
            from_bytes(request.body(), serde_querystring::ParseMode::UrlEncoded)?;

        let id: anyhow::Result<i32> = data.get("id").parse_into();
        let position: anyhow::Result<i32> = data.get("position").parse_into();

        match (data.get("action").map(|s| s.as_str()), id, position) {
            (Some("add"), ..) => {
                let dest = data.get("url").ok_or(anyhow!("No URL provided"))?;
                let title = data.get("title").ok_or(anyhow!("No title provided!"))?;
                query!("INSERT INTO external_links(title, destination, position, site_id) VALUES($1, $2, (SELECT COUNT(*) FROM external_links), $3)", title, dest, globals.site_id)
					.execute(&globals.connection_pool).await?;
            }
            (Some("delete"), Ok(id), _) => {
                query!(
                    "DELETE FROM external_links WHERE id=$1 AND site_id=$2",
                    id,
                    globals.site_id
                )
                .execute(&globals.connection_pool)
                .await?;
            }
            (Some("up"), Ok(id), Ok(position)) => {
                if position >= 0 {
                    query!(
                        "UPDATE external_links SET position = position + 1 WHERE position = $1 AND site_id=$2",
                        position - 1,
							globals.site_id
                    )
                    .execute(&globals.connection_pool)
                    .await?;
                    query!(
                        "UPDATE external_links SET position = position - 1 WHERE id = $1 AND site_id=$2",
                        id, globals.site_id
                    )
                    .execute(&globals.connection_pool)
                    .await?;
                }
            }
            (Some("down"), Ok(id), Ok(position)) => {
                let count = query!(
                    "SELECT count(*) AS count FROM external_links WHERE site_id=$1",
                    globals.site_id
                )
                .fetch_one(&globals.connection_pool)
                .await?;
                let new_position: i64 = (position + 1).into();
                if new_position < count.count.unwrap_or(0) {
                    query!(
                        "UPDATE external_links SET position = position - 1 WHERE position = $1 AND site_id=$2",
                        position + 1, globals.site_id
                    )
                    .execute(&globals.connection_pool)
                    .await?;
                    query!(
                        "UPDATE external_links SET position = position + 1 WHERE id = $1 AND site_id=$2",
                        id, globals.site_id
                    )
                    .execute(&globals.connection_pool)
                    .await?;
                }
            }
            _ => (),
        }
    }

    let links = query_as!(
        Link,
        "SELECT id, title, destination, position FROM external_links WHERE site_id=$1 ORDER BY position", globals.site_id
    )
    .fetch_all(&globals.connection_pool)
    .await?;

    let common = get_common(&globals, AdminMenuPages::Links).await?;
    let page = Links { common, links };

    render_html(page)
}
