use anyhow::anyhow;
use chrono::{Datelike, Month, NaiveDate, Utc};
use num_traits::FromPrimitive;
use serde::Serialize;
use tera::Context;
use tokio::{
    fs::{create_dir_all, File},
    io::AsyncWriteExt,
};

use crate::types::{CommonData, HydratedPost};

use super::types::Generator;

#[derive(Serialize)]
struct MonthIndexPage<'a> {
    title: &'a str,
    posts: Vec<&'a HydratedPost>,
    common: &'a CommonData,
    date: &'a NaiveDate,
}

pub async fn generate_month_index_pages(
    posts: &[HydratedPost],
    generator: &Generator<'_>,
) -> anyhow::Result<()> {
    let post_date = posts.last().ok_or(anyhow!("No posts!"))?.post_date;

    let mut current_date = NaiveDate::from_ymd_opt(post_date.year(), post_date.month(), 1).unwrap();
    let now = Utc::now().date_naive();

    while current_date <= now {
        let month_name = Month::from_u32(current_date.month())
            .ok_or(anyhow!("Bad month number"))?
            .name();
        let index_dir = format!(
            "{}/{}/{}",
            generator.output_path,
            current_date.year(),
            month_name
        );
        create_dir_all(&index_dir).await?;

        let mut file = File::create(format!("{}/{}", index_dir, "index.html")).await?;
        let mut month_posts: Vec<&HydratedPost> = posts
            .iter()
            .filter(|p| {
                let post_date = p.post_date.with_timezone(&generator.common.timezone);
                post_date.year() == current_date.year() && post_date.month() == current_date.month()
            })
            .collect();
        month_posts.sort_by(|a, b| a.post_date.cmp(&b.post_date));

        let title = format!("{} {}", month_name, current_date.year());
        let page = MonthIndexPage {
            title: title.as_str(),
            posts: month_posts,
            common: generator.common,
            date: &current_date,
        };

        let rendered = generator
            .tera
            .render("month_index.html", &Context::from_serialize(page)?)?;
        file.write_all(rendered.as_bytes()).await?;

        current_date = current_date
            .checked_add_months(chrono::Months::new(1))
            .unwrap();
    }

    Ok(())
}
