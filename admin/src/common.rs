use sqlx::{query, PgConnection};

use crate::types;

pub struct Common {
    pub current_page: types::AdminMenuPages,
    pub comments_waiting: i64,
    pub media_base_url: String,
}

pub async fn get_common(
    connection: &mut PgConnection,
    current_page: types::AdminMenuPages,
) -> anyhow::Result<Common> {
    let comment_count = query!("SELECT COUNT(*) AS count FROM comments WHERE status='pending'")
        .fetch_one(&mut *connection)
        .await?;

    let media_root = query!("SELECT value FROM blog_settings WHERE setting_name='media_base_url'")
        .fetch_optional(&mut *connection)
        .await?;

    Ok(Common {
        current_page,
        comments_waiting: comment_count.count.unwrap_or_default(),
        media_base_url: media_root.map(|m| m.value).unwrap_or_default(),
    })
}
