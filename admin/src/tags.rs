use askama::Template;
use cgi::http::Method;
use serde::Deserialize;
use shared::utils::{post_body, render_html};
use sqlx::{query, query_as};

use crate::{
    common::{Common, get_common},
    types::{AdminMenuPages, PageGlobals},
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

pub async fn render(request: &cgi::Request, globals: PageGlobals) -> anyhow::Result<cgi::Response> {
    if request.method() == Method::POST {
        let data: TagRequest = post_body(request)?;
        match data {
            TagRequest::Create(tag) => {
                query!(
                    "INSERT INTO tags(name, site_id) VALUES ($1, $2)",
                    tag.new,
                    globals.site_id
                )
                .execute(&globals.connection_pool)
                .await?;
            }
            TagRequest::Delete(tag) => {
                query!(
                    "DELETE FROM tags WHERE id=$1 AND site_id=$2",
                    tag.delete,
                    globals.site_id
                )
                .execute(&globals.connection_pool)
                .await?;
            }
        }
    }

    let tags = query_as!(
        DisplayTag,
        "SELECT id, name FROM tags WHERE site_id=$1 ORDER BY name",
        globals.site_id
    )
    .fetch_all(&globals.connection_pool)
    .await?;
    let tmpl = TagsPage {
        common: get_common(&globals, AdminMenuPages::Tags).await?,
        tags,
    };

    render_html(tmpl)
}
