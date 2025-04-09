use anyhow::anyhow;
use chrono::{Datelike, Month, NaiveDate, Utc};
use itertools::Itertools;
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
struct YearIndexPage<'a> {
    title: &'a str,
    posts_by_month: Vec<(Month, Vec<&'a HydratedPost>)>,
    common: &'a CommonData,
    date: &'a NaiveDate,
}

pub async fn generate_year_index_pages<'a>(
    posts: &Vec<HydratedPost>,
    generator: &Generator<'a>,
) -> anyhow::Result<()> {
    let post_date = posts.last().ok_or(anyhow!("No posts!"))?.post_date;

    let mut current_date = NaiveDate::from_ymd_opt(post_date.year(), 1, 1).unwrap();
    let now = Utc::now().date_naive();

    while current_date <= now {
        let index_dir = format!("{}/{}", generator.output_path, current_date.year());
        create_dir_all(&index_dir).await?;

        let mut file = File::create(format!("{}/{}", index_dir, "index.html")).await?;
        let mut month_posts: Vec<&HydratedPost> = posts
            .iter()
            .filter(|p| {
                p.post_date.with_timezone(&generator.common.timezone).year() == current_date.year()
            })
            .collect();
        month_posts.sort_by(|a, b| a.post_date.cmp(&b.post_date));
        let mut grouped = Vec::new();
        for (key, group) in &month_posts
            .into_iter()
            .group_by(|p| Month::from_u32(p.post_date.month()).unwrap())
        {
            grouped.push((key, group.collect()));
        }

        let title = format!("{}", current_date.year());
        let page = YearIndexPage {
            title: title.as_str(),
            posts_by_month: grouped,
            common: generator.common,
            date: &current_date,
        };

        let rendered = generator
            .tera
            .render("year_index.html", &Context::from_serialize(page)?)?;
        file.write_all(rendered.as_bytes()).await?;

        current_date = current_date
            .checked_add_months(chrono::Months::new(12))
            .unwrap();
    }

    Ok(())
}
