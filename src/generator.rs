use crate::database;
use crate::response::redirect_response;
use crate::session;

use askama::Template;
use async_std::fs::create_dir_all;
use sqlx::{query, query_as};
use time::{OffsetDateTime, Month};
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::prelude::*;
use itertools::Itertools;

pub struct HydratedPost {
    pub post_date: time::OffsetDateTime,
    pub url_slug: String,
    pub title: String,
    pub body: String,
    pub author_name: Option<String>,
}

pub struct Link {
	pub title: String,
	pub destination: String,
}

pub struct CommonData {
    base_url: String,
    blog_name: String,
    archive_years: Vec<i32>,
	links: Vec<Link>,
}

#[derive(Template)]
#[template(path = "generated/post.html")]
struct PostPage<'a> {
    title: &'a String,
    post: &'a HydratedPost,
    common: &'a CommonData,
}

#[derive(Template)]
#[template(path = "generated/index.html")]
struct IndexPage<'a> {
    title: &'a str,
    common: &'a CommonData,
    posts: &'a [HydratedPost],
}

#[derive(Template)]
#[template(path = "generated/month_index.html")]
struct MonthIndexPage<'a> {
    title: &'a str,
    posts: Vec<&'a HydratedPost>,
    common: &'a CommonData,
	date: &'a OffsetDateTime,
}

#[derive(Template)]
#[template(path = "generated/year_index.html")]
struct YearIndexPage<'a> {
    title: &'a str,
    posts_by_month: Vec<(Month, Vec<&'a HydratedPost>)>,
    common: &'a CommonData,
	date: &'a OffsetDateTime,
}

mod filters {
    use super::{HydratedPost, CommonData};
	use ordinal::Ordinal;
    use time::{
        format_description::{self, well_known::Rfc3339},
        OffsetDateTime,
    };

    pub fn posturl(post: &HydratedPost, common: &CommonData) -> ::askama::Result<String> {
        let url = format!(
            "{}{}/{}/{}.html",
			common.base_url,
            post.post_date.year(),
            post.post_date.month(),
            post.url_slug
        );
        Ok(url)
    }

    pub fn format_human_date(date_time: &OffsetDateTime) -> ::askama::Result<String> {
        let format = format_description::parse("[weekday], [day] [month repr:long] [year]")
            .map_err(|_| ::askama::Error::Custom("".into()))?;
        date_time
            .format(&format)
            .map_err(|e| ::askama::Error::Custom(e.into()))
    }

    pub fn format_rfc3339_date(date_time: &OffsetDateTime) -> ::askama::Result<String> {
        date_time
            .format(&Rfc3339)
            .map_err(|e| ::askama::Error::Custom(e.into()))
    }

	pub fn format_weekday(date_time: &OffsetDateTime) -> ::askama::Result<String> {
		let weekday = date_time.weekday();
		let day = Ordinal(date_time.day());
		Ok(format!("{} {}", weekday, day))
	}
}

pub async fn regenerate_blog(request: &cgi::Request) -> anyhow::Result<cgi::Response> {
    let mut connection = database::connect_db().await?;
    session::session_id(&mut connection, &request).await?;
    let output_path = env::var("BLOG_OUTPUT_PATH").expect("Environment variable BLOG_OUTPUT_PATH is required");

    let raw_settings = query!("SELECT setting_name, value FROM blog_settings")
        .fetch_all(&mut connection)
        .await?;
    let settings: HashMap<String, String> =
        HashMap::from_iter(raw_settings.into_iter().map(|r| (r.setting_name, r.value)));

	let links = query_as!(Link, "SELECT title, destination FROM external_links ORDER BY position").fetch_all(&mut connection).await?;

    let posts = query_as!(
        HydratedPost,
        "
SELECT post_date, url_slug, title, body, users.display_name AS author_name
FROM posts
INNER JOIN users
ON users.id = posts.author_id
ORDER BY post_date DESC"
    )
    .fetch_all(&mut connection)
    .await?;

    if posts.len() == 0 {
        return Ok(redirect_response("dashboard"));
    }

	let earliest_year = posts.last().unwrap().post_date.year();
    let current_year = time::OffsetDateTime::now_utc().year();
	let mut years:  Vec<i32> = (earliest_year..=current_year).collect();
	years.reverse();

    let common = CommonData {
        base_url: settings.get("base_url").unwrap().to_owned(),
        blog_name: settings.get("blog_name").unwrap().to_owned(),
        archive_years: years,
		links
    };
    for post in &posts {
        let dir = format!(
            "{}/{}/{}",
            output_path,
            post.post_date.year(),
            post.post_date.month(),
        );
        let post_path = format!("{}/{}.html", dir, post.url_slug);
        create_dir_all(dir).await?;

        let mut file = File::create(post_path)?;
        let post_page = PostPage {
            title: &post.title,
            post,
			common: &common
        };

        let rendered = post_page.render()?;
        write!(&mut file, "{}", rendered)?;
    }

    for (pos, chunk) in posts.chunks(10).enumerate() {
        let path = if pos == 0 {
            String::from("index.html")
        } else {
            format!("{}.html", pos + 1)
        };
        let mut file = File::create(format!("{}/{}", output_path, path))?;

        let page = IndexPage {
            title: &common.blog_name,
            posts: chunk,
            common: &common,
        };

        let rendered = page.render()?;
        write!(&mut file, "{}", rendered)?;
    }

    regenerate_month_index_pages(&output_path, &posts, &common).await?;
    regenerate_year_index_pages(&output_path, &posts, &common).await?;

    Ok(redirect_response("dashboard"))
}

async fn regenerate_month_index_pages(
    output_path: &String,
    posts: &Vec<HydratedPost>,
    common: &CommonData,
) -> anyhow::Result<()> {
    let mut current_date = posts.last().unwrap().post_date.replace_day(1).unwrap();
    let now = time::OffsetDateTime::now_utc();

    while current_date <= now {
        let index_dir = format!(
            "{}/{}/{}",
            output_path,
            current_date.year(),
            current_date.month()
        );

        let mut file = File::create(format!("{}/{}", index_dir, "index.html"))?;
        let mut month_posts: Vec<&HydratedPost> = posts
            .iter()
            .filter(|p| {
                p.post_date.year() == current_date.year()
                    && p.post_date.month() == current_date.month()
            })
            .collect();
        month_posts.sort_by(|a, b| a.post_date.cmp(&b.post_date));

        let title = format!("{} {}", current_date.month(), current_date.year());
        let page = MonthIndexPage {
            title: title.as_str(),
            posts: month_posts,
            common,
			date: &current_date
        };

        let rendered = page.render()?;
        write!(&mut file, "{}", rendered)?;

        let days_to_add = time::util::days_in_year_month(current_date.year(), current_date.month());
        current_date = current_date + time::Duration::days(days_to_add.into());
    }

    Ok(())
}

async fn regenerate_year_index_pages(
    output_path: &String,
    posts: &Vec<HydratedPost>,
    common: &CommonData,
) -> anyhow::Result<()> {
    let mut current_date = posts.last().unwrap().post_date.replace_day(1).unwrap();
    let now = time::OffsetDateTime::now_utc();

    while current_date <= now {
        let index_dir = format!("{}/{}", output_path, current_date.year());
        create_dir_all(&index_dir).await?;

        let mut file = File::create(format!("{}/{}", index_dir, "index.html"))?;
        let mut month_posts: Vec<&HydratedPost> = posts
            .iter()
            .filter(|p| p.post_date.year() == current_date.year())
            .collect();
        month_posts.sort_by(|a, b| a.post_date.cmp(&b.post_date));
		let mut grouped = Vec::new();
		for (key, group) in &month_posts.into_iter().group_by(|p| p.post_date.month()) {
			grouped.push((key, group.collect()));
		}


        let title = format!("{}", current_date.year());
        let page = YearIndexPage {
            title: title.as_str(),
            posts_by_month: grouped ,
            common,
			date: &current_date

        };

        let rendered = page.render()?;
        write!(&mut file, "{}", rendered)?;

        let days_to_add = time::util::days_in_year(current_date.year());
        current_date = current_date + time::Duration::days(days_to_add.into());
    }

    Ok(())
}
