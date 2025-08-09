use bsky::{BSky, Embeds, External, Post};
use shared::{
    database::connect_db,
    settings::{Settings, get_settings_struct},
    utils::blog_post_url,
};
use sqlx::{PgPool, query};

mod bsky;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let connection = connect_db().await?;
    let site_ids = query!("SELECT id FROM sites")
        .fetch_all(&connection)
        .await?;
    for site in site_ids {
        send_posts_for_site(&connection, site.id).await?;
    }
    Ok(())
}

async fn send_posts_for_site(connection: &PgPool, site_id: i32) -> Result<(), anyhow::Error> {
    let settings = get_settings_struct(connection, site_id).await?;
    let bsky_url = reqwest::Url::parse("https://bsky.social")?;
    if let (Some(bsky_username), Some(bsky_password)) = (
        settings.bsky_username.clone(),
        settings.bsky_password.clone(),
    ) {
        let bsky = BSky::login(&bsky_url, &bsky_username, &bsky_password).await?;

        let ids_to_post = find_posts(connection).await?;
        for post_id in ids_to_post {
            post_to_bsky(connection, &settings, post_id, site_id, &bsky).await?;
        }
    }
    Ok(())
}

async fn post_to_bsky(
    connection: &PgPool,
    settings: &Settings,
    post_id: i32,
    site_id: i32,
    agent: &BSky,
) -> Result<(), anyhow::Error> {
    let post = query!("SELECT id, title, summary, url_slug, post_date FROM posts p WHERE p.id = $1 AND p.site_id = $2", post_id, site_id).fetch_one(connection).await?;

    let url = blog_post_url(
        post.url_slug,
        post.post_date,
        settings.timezone,
        settings.base_url.clone(),
    )?;

    let summary = match post.summary {
        Some(x) if !x.is_empty() => x,
        _ => "I forgot to add a summary.".into(),
    };

    let text = format!("A new post has been published! \r\n{}", post.title,);

    let embed = Embeds::External(External::new(url, post.title, summary, None));

    query!(
        "INSERT INTO bsky_outbox(post_id, posted_at) VALUES ($1, CURRENT_TIMESTAMP)",
        Some::<i64>(post.id.into())
    )
    .execute(connection)
    .await?;
    agent.new_post(Post::new(text, Some(embed))).await?;

    Ok(())
}

async fn find_posts(connection: &PgPool) -> Result<Vec<i32>, anyhow::Error> {
    let to_post = query!(
        r#"
SELECT p.id
FROM posts p
WHERE p.state='published'
AND NOT EXISTS (SELECT b.id FROM bsky_outbox b WHERE b.post_id = p.id)
ORDER BY p.post_date DESC"#
    )
    .fetch_all(connection)
    .await?;
    let ids = to_post.into_iter().map(|r| r.id).collect();

    Ok(ids)
}
