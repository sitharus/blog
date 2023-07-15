use std::env;
use tokio::runtime::Runtime;

use sqlx::postgres::PgConnection;
use sqlx::Connection;

fn main() {
    let runtime = Runtime::new().unwrap();
    runtime.block_on(migrate());
}

async fn migrate() {
    let connection_string = env::var("BLOG_CONNECTION_STRING")
        .expect("Environment variable BLOG_CONNECTION_STRING must be set");
    let mut connection = PgConnection::connect(&connection_string).await.unwrap();
    sqlx::migrate!("../db/migrations")
        .run(&mut connection)
        .await
        .unwrap();
}
