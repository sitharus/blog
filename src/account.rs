use crate::types::AdminMenuPages;
use crate::utils::post_body;
use crate::utils::render_html;
use askama::Template;
use cgi;
use serde::Deserialize;
use sqlx::query;

use super::database;
use super::session;

#[derive(Template)]
#[template(path = "account.html")]
struct Account {
    selected_menu_item: AdminMenuPages,
    name: String,
}

#[derive(Deserialize)]
struct FormContent {
    target: String,
    display_name: Option<String>,
    current_password: Option<String>,
    new_password: Option<String>,
    repeat_new_password: Option<String>,
}

pub async fn render(request: &cgi::Request) -> anyhow::Result<cgi::Response> {
    let mut connection = database::connect_db().await?;
    let session = session::session_id(&mut connection, &request).await?;

    if request.method() == "POST" {
        let form: FormContent = post_body(request)?;
        match (form.target.as_str(), &form) {
            (
                "details",
                FormContent {
                    display_name: Some(name),
                    ..
                },
            ) => {
                query!(
                    "UPDATE users SET display_name = $1 WHERE id = $2",
                    name,
                    session.user_id
                )
                .execute(&mut connection)
                .await?;
            }
            (
                "password",
                FormContent {
                    current_password: Some(current_password),
                    new_password: Some(new_password),
                    repeat_new_password: Some(repeat_new_password),
                    ..
                },
            ) if new_password == repeat_new_password => {
                let existing_password =
                    query!("SELECT password FROM users WHERE id=$1", session.user_id)
                        .fetch_one(&mut connection)
                        .await
                        .unwrap();
                bcrypt::verify(&current_password.as_bytes(), &existing_password.password)?;
                let new_hash = bcrypt::hash(new_password, bcrypt::DEFAULT_COST)?;
                query!(
                    "UPDATE users SET password = $1 WHERE id = $2",
                    new_hash,
                    session.user_id
                )
                .execute(&mut connection)
                .await?;
            }

            _ => (),
        };
    }

    let details = query!(
        "SELECT display_name FROM users WHERE id=$1",
        session.user_id
    )
    .fetch_one(&mut connection)
    .await?;

    let content = Account {
        selected_menu_item: AdminMenuPages::Account,
        name: details.display_name.unwrap_or("".into()),
    };

    render_html(content)
}
