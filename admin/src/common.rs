use shared::settings::{get_settings_struct, Settings};
use sqlx::query;

use crate::types::{self, PageGlobals};

pub struct Common {
    pub current_page: types::AdminMenuPages,
    pub comments_waiting: i64,
    pub media_base_url: String,
    pub settings: Settings,
}

pub async fn get_common<'c>(
    globals: &PageGlobals,
    current_page: types::AdminMenuPages,
) -> anyhow::Result<Common> {
    let comment_count = query!("SELECT COUNT(*) AS count FROM comments INNER JOIN posts ON comments.post_id = posts.id WHERE comments.status='pending' AND posts.site_id=$1", globals.site_id)
        .fetch_one(&globals.connection_pool)
        .await?;

    let settings = get_settings_struct(&globals.connection_pool, globals.site_id).await?;
    let media_base_url = settings.media_base_url.clone();

    Ok(Common {
        current_page,
        comments_waiting: comment_count.count.unwrap_or_default(),
        media_base_url,
        settings,
    })
}
