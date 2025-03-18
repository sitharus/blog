use sqlx::PgPool;
use tera::Tera;

use crate::types::CommonData;

pub struct Generator<'a> {
    pub output_path: &'a str,
    pub common: &'a CommonData,
    pub pool: &'a PgPool,
    pub tera: Tera,
    pub site_id: i32,
}
