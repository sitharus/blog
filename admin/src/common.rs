use sqlx::{query, PgConnection};

use crate::types;

pub struct Common {
    pub current_page: types::AdminMenuPages,
    pub comments_waiting: i64,
}

pub async fn get_common(
    connection: &mut PgConnection,
    current_page: types::AdminMenuPages,
) -> anyhow::Result<Common> {
    let comment_count = query!("SELECT COUNT(*) AS count FROM comments WHERE status='pending'")
        .fetch_one(connection)
        .await?;

    Ok(Common {
        current_page,
        comments_waiting: comment_count.count.unwrap_or_default(),
    })
}
