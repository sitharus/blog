use askama::Template;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use shared::generator::templates::default_templates;
use shared::utils::{post_body, render_html};
use std::ops::Bound;
use std::ops::Bound::{Excluded, Included, Unbounded};

use crate::{filters, response};

use crate::{
    common::{get_common, Common},
    types::PageGlobals,
};

struct TemplateListItem<'a> {
    name: &'a str,
    customised: bool,
    last_edit: Option<DateTime<Utc>>,
}

#[derive(Template)]
#[template(path = "templates.html")]
struct TemplatesList<'a> {
    common: Common,
    templates: Vec<TemplateListItem<'a>>,
}

#[derive(Template)]
#[template(path = "edit_template.html")]
struct EditTemplate {
    common: Common,
    kind: String,
    content: String,
}

pub async fn templates(
    _request: &cgi::Request,
    globals: PageGlobals,
) -> anyhow::Result<cgi::Response> {
    let common = get_common(&globals, crate::types::AdminMenuPages::Templates).await?;
    let default_templates = default_templates();
    let customised = sqlx::query!(
        "SELECT id, template_kind, sys_period FROM templates WHERE site_id=$1",
        globals.site_id,
    )
    .fetch_all(&globals.connection_pool)
    .await?;

    let templates = default_templates
        .iter()
        .map(|entry| {
            let custom = customised.iter().find(|f| &f.template_kind == entry.0);
            TemplateListItem {
                name: entry.0,
                customised: custom.is_some(),
                last_edit: custom.and_then(|c| bound_option(c.sys_period.start)),
            }
        })
        .collect();
    render_html(TemplatesList { common, templates })
}

fn bound_option(bound: Bound<DateTime<Utc>>) -> Option<DateTime<Utc>> {
    match bound {
        Included(x) => Some(x),
        Excluded(_) => None,
        Unbounded => None,
    }
}

#[derive(Deserialize)]
struct TemplateUpdate {
    kind: String,
    content: String,
}

pub async fn edit_template(
    request: &cgi::Request,
    globals: PageGlobals,
) -> anyhow::Result<cgi::Response> {
    if request.method() == "POST" {
        let body: TemplateUpdate = post_body(request)?;
        sqlx::query!(
            "
INSERT INTO templates(site_id, created_by, template_kind, content)
VALUES ($1, $2, $3, $4)
ON CONFLICT (site_id, template_kind) DO UPDATE SET created_by=$2, content=$4
",
            globals.site_id,
            globals.session.user_id,
            body.kind,
            body.content,
        )
        .execute(&globals.connection_pool)
        .await?;
        Ok(response::redirect_response("templates"))
    } else {
        let template_name = globals
            .query
            .get("template_name")
            .ok_or(anyhow::anyhow!("No template specified"))?;
        let custom_template = sqlx::query!(
            "SELECT id, content FROM templates WHERE template_kind=$1 AND site_id=$2",
            template_name,
            globals.site_id
        )
        .fetch_optional(&globals.connection_pool)
        .await?;

        let template = custom_template
            .map(|t| t.content)
            .or_else(|| {
                default_templates()
                    .get(template_name)
                    .map(|t| t.contents.clone())
            })
            .ok_or(anyhow::anyhow!("Invalid template name"))?;

        render_html(EditTemplate {
            common: get_common(&globals, crate::types::AdminMenuPages::Templates).await?,
            kind: template_name.to_string(),
            content: template,
        })
    }
}
