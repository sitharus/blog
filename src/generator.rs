use crate::database;
use crate::response::redirect_response;
use crate::session;

use anyhow::anyhow;
use askama::Template;
use async_std::fs::create_dir_all;
use chrono::{offset::Utc, DateTime, Datelike, Month, NaiveDate};
use itertools::Itertools;
use num_traits::FromPrimitive;
use sqlx::{query, query_as};
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::prelude::*;

pub struct HydratedPost {
    pub id: i32,
    pub post_date: NaiveDate,
    pub url_slug: String,
    pub title: String,
    pub body: String,
    pub author_name: Option<String>,
    pub comment_count: Option<i64>,
}

pub struct HydratedComment {
    author_name: String,
    created_date: DateTime<Utc>,
    post_body: String,
}

pub struct Link {
    pub title: String,
    pub destination: String,
}

pub struct CommonData {
    base_url: String,
    static_base_url: String,
    comment_cgi_url: String,
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
    comments: Vec<HydratedComment>,
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
    date: &'a NaiveDate,
}

#[derive(Template)]
#[template(path = "generated/year_index.html")]
struct YearIndexPage<'a> {
    title: &'a str,
    posts_by_month: Vec<(Month, Vec<&'a HydratedPost>)>,
    common: &'a CommonData,
    date: &'a NaiveDate,
}

#[derive(Template)]
#[template(path = "generated/feed.xml")]
struct RssFeed<'a> {
    common: &'a CommonData,
    posts: &'a [HydratedPost],
    date: DateTime<Utc>,
}

#[derive(Template)]
#[template(path = "generated/atom.xml")]
struct AtomFeed<'a> {
    common: &'a CommonData,
    posts: &'a [HydratedPost],
    date: DateTime<Utc>,
}

mod filters {
    use super::{CommonData, HydratedPost};
    use chrono::{offset::Utc, DateTime, Datelike, Month, NaiveDate};
    use num_traits::FromPrimitive;
    use ordinal::Ordinal;

    pub fn posturl(post: &HydratedPost, common: &CommonData) -> ::askama::Result<String> {
        let month = Month::from_u32(post.post_date.month())
            .ok_or(::askama::Error::Custom("Could not find month".into()))?
            .name();
        let url = format!(
            "{}{}/{}/{}.html",
            common.base_url,
            post.post_date.year(),
            month,
            post.url_slug
        );
        Ok(url)
    }

    pub fn month_name(month: u32) -> ::askama::Result<String> {
        let month = Month::from_u32(month)
            .ok_or(::askama::Error::Custom("Could not find month".into()))?
            .name();
        Ok(String::from(month))
    }

    pub fn format_human_date(date_time: &NaiveDate) -> ::askama::Result<String> {
        Ok(date_time.format("%A, %-d %B, %C%y").to_string())
    }

    pub fn format_human_datetime(date_time: &DateTime<Utc>) -> ::askama::Result<String> {
        Ok(date_time.format("%A, %-d %B, %C%y at %-I:%m%P").to_string())
    }

    pub fn format_rfc3339_datetime(date_time: &DateTime<Utc>) -> ::askama::Result<String> {
        Ok(date_time.to_rfc3339())
    }

    pub fn format_rfc2822_datetime(date_time: &DateTime<Utc>) -> ::askama::Result<String> {
        Ok(date_time.to_rfc2822())
    }

    pub fn format_rfc3339_date(date: &NaiveDate) -> ::askama::Result<String> {
        date.and_hms_opt(0, 0, 0)
            .ok_or(::askama::Error::Custom(
                "Could not find midnight UTC".into(),
            ))?
            .and_local_timezone(Utc)
            .earliest()
            .ok_or(::askama::Error::Custom("Cannot convert to UTC".into()))
            .map(|d| d.to_rfc3339())
    }

    pub fn format_rfc2822_date(date: &NaiveDate) -> ::askama::Result<String> {
        date.and_hms_opt(0, 0, 0)
            .ok_or(::askama::Error::Custom(
                "Could not find midnight UTC".into(),
            ))?
            .and_local_timezone(Utc)
            .earliest()
            .ok_or(::askama::Error::Custom("Cannot convert to UTC".into()))
            .map(|d| d.to_rfc2822())
    }

    pub fn pluralise(base: &str, count: &Option<i64>) -> ::askama::Result<String> {
        match count {
            Some(1) => Ok(base.to_string()),
            _ => Ok(format!("{}s", base)),
        }
    }

    pub fn format_weekday(date: &NaiveDate) -> ::askama::Result<String> {
        let weekday = date.weekday();
        let day = Ordinal(date.day());
        Ok(format!("{} {}", weekday, day))
    }
}

pub async fn regenerate_blog(request: &cgi::Request) -> anyhow::Result<cgi::Response> {
    let mut connection = database::connect_db().await?;
    session::session_id(&mut connection, &request).await?;
    let output_path =
        env::var("BLOG_OUTPUT_PATH").expect("Environment variable BLOG_OUTPUT_PATH is required");

    let raw_settings = query!("SELECT setting_name, value FROM blog_settings")
        .fetch_all(&mut connection)
        .await?;
    let settings: HashMap<String, String> =
        HashMap::from_iter(raw_settings.into_iter().map(|r| (r.setting_name, r.value)));

    let links = query_as!(
        Link,
        "SELECT title, destination FROM external_links ORDER BY position"
    )
    .fetch_all(&mut connection)
    .await?;

    let posts = query_as!(
        HydratedPost,
        "
SELECT posts.id as id, post_date, url_slug, title, body, users.display_name AS author_name, (SELECT COUNT(*) FROM comments WHERE comments.post_id = posts.id) AS comment_count
FROM posts
INNER JOIN users
ON users.id = posts.author_id
WHERE state = 'published'
ORDER BY post_date DESC"
    )
    .fetch_all(&mut connection)
    .await?;

    if posts.len() == 0 {
        return Ok(redirect_response("dashboard"));
    }

    let earliest_year = posts.last().ok_or(anyhow!("No posts!"))?.post_date.year();
    let current_year = Utc::now().year();
    let mut years: Vec<i32> = (earliest_year..=current_year).collect();
    years.reverse();

    let common = CommonData {
        base_url: settings
            .get("base_url")
            .ok_or(anyhow!("No blog URL set"))?
            .to_owned(),
        blog_name: settings
            .get("blog_name")
            .ok_or(anyhow!("No blog name set"))?
            .to_owned(),
        static_base_url: settings
            .get("static_base_url")
            .ok_or(anyhow!("No base URL set"))?
            .to_owned(),
        comment_cgi_url: settings
            .get("comment_cgi_url")
            .ok_or(anyhow!("No CGI url set"))?
            .to_owned(),
        archive_years: years,
        links,
    };
    for post in &posts {
        generate_post_page(&output_path, post, &common, &mut connection).await?;
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
    regenerate_rss_feed(&output_path, &posts, &common).await?;
    regenerate_atom_feed(&output_path, &posts, &common).await?;

    Ok(redirect_response("dashboard"))
}

async fn generate_post_page(
    output_path: &String,
    post: &HydratedPost,
    common: &CommonData,
    connection: &mut sqlx::PgConnection,
) -> anyhow::Result<()> {
    let comments = query_as!(HydratedComment, "SELECT author_name, post_body, created_date FROM comments WHERE post_id=$1 AND status = 'approved' ORDER BY created_date ASC", post.id).fetch_all(connection).await?;

    let month_name = Month::from_u32(post.post_date.month())
        .ok_or(anyhow!("Bad month number"))?
        .name();
    let dir = format!("{}/{}/{}", output_path, post.post_date.year(), month_name);
    let post_path = format!("{}/{}.html", dir, post.url_slug);
    create_dir_all(dir).await?;

    let mut file = File::create(post_path)?;
    let post_page = PostPage {
        title: &post.title,
        post,
        common: &common,
        comments,
    };

    let rendered = post_page.render()?;
    write!(&mut file, "{}", rendered)?;
    Ok(())
}

async fn regenerate_rss_feed(
    output_path: &String,
    posts: &Vec<HydratedPost>,
    common: &CommonData,
) -> anyhow::Result<()> {
    let max = ::std::cmp::min(posts.len(), 10);
    let posts_in_feed = posts
        .get(0..max)
        .ok_or(anyhow!("Failed to get posts for feed"))?;

    let feed = RssFeed {
        common,
        posts: posts_in_feed,
        date: Utc::now(),
    };
    let mut file = File::create(format!("{}/feed.rss", output_path))?;
    let rendered = feed.render()?;
    write!(&mut file, "{}", rendered)?;

    Ok(())
}

async fn regenerate_atom_feed(
    output_path: &String,
    posts: &Vec<HydratedPost>,
    common: &CommonData,
) -> anyhow::Result<()> {
    let max = ::std::cmp::min(posts.len(), 10);
    let posts_in_feed = posts
        .get(0..max)
        .ok_or(anyhow!("Failed to get posts for feed"))?;

    let feed = AtomFeed {
        common,
        posts: posts_in_feed,
        date: Utc::now(),
    };
    let mut file = File::create(format!("{}/feed.atom", output_path))?;
    let rendered = feed.render()?;
    write!(&mut file, "{}", rendered)?;

    Ok(())
}
async fn regenerate_month_index_pages(
    output_path: &String,
    posts: &Vec<HydratedPost>,
    common: &CommonData,
) -> anyhow::Result<()> {
    let post_date = posts.last().ok_or(anyhow!("No posts!"))?.post_date;

    let mut current_date = NaiveDate::from_ymd_opt(post_date.year(), post_date.month(), 1).unwrap();
    let now = Utc::now().date_naive();

    while current_date <= now {
        let month_name = Month::from_u32(current_date.month())
            .ok_or(anyhow!("Bad month number"))?
            .name();
        let index_dir = format!("{}/{}/{}", output_path, current_date.year(), month_name);

        let mut file = File::create(format!("{}/{}", index_dir, "index.html"))?;
        let mut month_posts: Vec<&HydratedPost> = posts
            .iter()
            .filter(|p| {
                p.post_date.year() == current_date.year()
                    && p.post_date.month() == current_date.month()
            })
            .collect();
        month_posts.sort_by(|a, b| a.post_date.cmp(&b.post_date));

        let title = format!("{} {}", month_name, current_date.year());
        let page = MonthIndexPage {
            title: title.as_str(),
            posts: month_posts,
            common,
            date: &current_date,
        };

        let rendered = page.render()?;
        write!(&mut file, "{}", rendered)?;

        current_date = current_date
            .checked_add_months(chrono::Months::new(1))
            .unwrap();
    }

    Ok(())
}

async fn regenerate_year_index_pages(
    output_path: &String,
    posts: &Vec<HydratedPost>,
    common: &CommonData,
) -> anyhow::Result<()> {
    let post_date = posts.last().ok_or(anyhow!("No posts!"))?.post_date;

    let mut current_date = NaiveDate::from_ymd_opt(post_date.year(), 1, 1).unwrap();
    let now = Utc::now().date_naive();

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
            common,
            date: &current_date,
        };

        let rendered = page.render()?;
        write!(&mut file, "{}", rendered)?;

        current_date = current_date
            .checked_add_months(chrono::Months::new(12))
            .unwrap();
    }

    Ok(())
}
