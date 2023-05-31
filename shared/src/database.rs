use std::env;

use sqlx::postgres::PgConnection;
use sqlx::Connection;

pub async fn connect_db() -> anyhow::Result<PgConnection, sqlx::Error> {
    let connection_string = env::var("BLOG_CONNECTION_STRING")
        .expect("Environment variable BLOG_CONNECTION_STRING must be set");
    PgConnection::connect(&connection_string).await
}
