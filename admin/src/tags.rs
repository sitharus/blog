use askama::Template;
use http::Method;
use serde::Deserialize;
use shared::{
    database,
    utils::{post_body, render_html},
};
use sqlx::{query, query_as};

use crate::{
    common::{get_common, Common},
    types::AdminMenuPages,
};

struct DisplayTag {
    id: i32,
    name: String,
}

#[derive(Deserialize)]
struct NewTag {
    new: String,
}

#[derive(Deserialize)]
struct DeleteTag {
    delete: i32,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum TagRequest {
    Delete(DeleteTag),
    Create(NewTag),
}

#[derive(Template)]
#[template(path = "tags.html")]
struct TagsPage {
    common: Common,
    tags: Vec<DisplayTag>,
}

pub async fn render(request: &cgi::Request) -> anyhow::Result<cgi::Response> {
    let mut connection = database::connect_db().await?;

    if request.method() == Method::POST {
        let data: TagRequest = post_body(request)?;
        match data {
            TagRequest::Create(tag) => {
                query!("INSERT INTO tags(name) VALUES ($1)", tag.new)
                    .execute(&mut connection)
                    .await?;
            }
            TagRequest::Delete(tag) => {
                query!("DELETE FROM tags WHERE id=$1", tag.delete)
                    .execute(&mut connection)
                    .await?;
            }
        }
    }

    let tags = query_as!(DisplayTag, "SELECT id, name FROM tags ORDER BY name")
        .fetch_all(&mut connection)
        .await?;
    let tmpl = TagsPage {
        common: get_common(&mut connection, AdminMenuPages::Tags).await?,
        tags,
    };

    render_html(tmpl)
}
