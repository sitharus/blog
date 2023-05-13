use std::collections::HashMap;

use askama::Template;
use serde_querystring::from_bytes;
use sqlx::{query, query_as};

use crate::{database, session, types::AdminMenuPages};

#[derive(Template)]
#[template(path = "links.html")]
struct Links {
    selected_menu_item: AdminMenuPages,
    links: Vec<Link>,
}

struct Link {
    id: i32,
    title: String,
    destination: String,
    position: i32,
}

pub async fn render(request: &cgi::Request) -> anyhow::Result<cgi::Response> {
    let mut connection = database::connect_db().await?;

    session::session_id(&mut connection, &request).await?;

    if request.method() == "POST" {
        let data: HashMap<String, String> =
            from_bytes(request.body(), serde_querystring::ParseMode::UrlEncoded)?;

		let id: Option<i32> = data.get("id").map(|i| i.parse().unwrap());
		let position: Option<i32> = data.get("position").map(|i| i.parse().unwrap());

        match (data.get("action").map(|s| s.as_str()), id, position) {
            (Some("add"), ..) => {
                let dest = data.get("url").unwrap();
                let title = data.get("title").unwrap();
                query!("INSERT INTO external_links(title, destination, position) VALUES($1, $2, (SELECT COUNT(*) FROM external_links))", title, dest).execute(&mut connection).await?;
            },
			(Some("delete"), Some(id), _) => {
				query!("DELETE FROM external_links WHERE id=$1", id).execute(&mut connection).await?;
			},
			(Some("up"), Some(id), Some(position)) => {
				if position >= 0 {
					query!("UPDATE external_links SET position = position + 1 WHERE position = $1", position-1).execute(&mut connection).await?;
					query!("UPDATE external_links SET position = position - 1 WHERE id = $1", id).execute(&mut connection).await?;
				}
			},
			(Some("down"), Some(id), Some(position)) => {
				let count = query!("SELECT count(*) AS count FROM external_links").fetch_one(&mut connection).await?;
				let new_position: i64 = (position + 1).into();
				if new_position < count.count.unwrap() {
					query!("UPDATE external_links SET position = position - 1 WHERE position = $1", position + 1).execute(&mut connection).await?;
					query!("UPDATE external_links SET position = position + 1 WHERE id = $1", id).execute(&mut connection).await?;
				}
			}
            _ => (),
        }
    }

    let links = query_as!(
        Link,
        "SELECT id, title, destination, position FROM external_links ORDER BY position"
    )
    .fetch_all(&mut connection)
    .await?;

    let page = Links {
        selected_menu_item: AdminMenuPages::Links,
        links,
    };

    Ok(cgi::html_response(200, page.render().unwrap()))
}
