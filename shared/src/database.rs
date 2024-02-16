use std::env;

use sqlx::PgPool;

pub async fn connect_db() -> anyhow::Result<PgPool, sqlx::Error> {
    let connection_string = env::var("BLOG_CONNECTION_STRING")
        .expect("Environment variable BLOG_CONNECTION_STRING must be set");
    PgPool::connect(&connection_string).await
}
