use std::collections::HashMap;

use shared::settings::{get_settings_struct, Settings};
use sqlx::{query, query_as};

use crate::types::{self, PageGlobals};

pub struct Site {
    pub id: i32,
    pub site_name: String,
    pub editions_enabled: bool,
}

pub struct Common {
    pub current_page: types::AdminMenuPages,
    pub comments_waiting: i64,
    pub media_base_url: String,
    pub settings: Settings,
    pub current_site_id: i32,
    pub sites: Vec<Site>,
    pub current_query: HashMap<String, String>,
    pub editions_enabled: bool,
}

pub async fn get_common(
    globals: &PageGlobals,
    current_page: types::AdminMenuPages,
) -> anyhow::Result<Common> {
    let comment_count = query!("SELECT COUNT(*) AS count FROM comments INNER JOIN posts ON comments.post_id = posts.id WHERE comments.status='pending' AND posts.site_id=$1", globals.site_id)
        .fetch_one(&globals.connection_pool)
        .await?;

    let sites = query_as!(
        Site,
        "SELECT id, site_name, editions_enabled FROM sites ORDER BY id"
    )
    .fetch_all(&globals.connection_pool)
    .await?;

    let editions_enabled = sites
        .iter()
        .find(|s| s.id == globals.site_id)
        .map(|s| s.editions_enabled)
        .unwrap_or_default();

    let settings = get_settings_struct(&globals.connection_pool, globals.site_id).await?;
    let media_base_url = settings.media_base_url.clone();

    Ok(Common {
        current_page,
        comments_waiting: comment_count.count.unwrap_or_default(),
        media_base_url,
        settings,
        current_site_id: globals.site_id,
        sites,
        current_query: globals.query.clone(),
        editions_enabled,
    })
}
