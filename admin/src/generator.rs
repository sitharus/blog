use crate::response::redirect_response;
use crate::types::{PageGlobals, PostRequest};
use shared::activities::Activity;
use shared::generator::{filters, get_common};
use shared::types::{CommonData, HydratedComment, HydratedPost};
use shared::utils::{post_body, render_html};

use anyhow::anyhow;
use askama::Template;
use chrono::{offset::Utc, DateTime, Datelike, Month, NaiveDate};
use itertools::Itertools;
use num_traits::FromPrimitive;
use sqlx::PgPool;
use sqlx::{query, query_as, types::Json};
use std::env;
use std::fs::File;
use std::io::prelude::*;
use std::vec::IntoIter;
use tokio::fs::create_dir_all;

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
    posts: Vec<&'a HydratedPost>,
    page: i32,
    total_pages: i32,
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

#[derive(Template)]
#[template(path = "generated/page.html")]
struct Page<'a> {
    common: &'a CommonData,
    title: String,
    body: String,
    last_updated: DateTime<Utc>,
}

pub async fn preview_page(
    request: &cgi::Request,
    globals: PageGlobals,
) -> anyhow::Result<cgi::Response> {
    let common = get_common(&globals.connection_pool, globals.site_id).await?;
    let user = query!(
        "SELECT display_name FROM users WHERE id=$1",
        globals.session.user_id
    )
    .fetch_one(&globals.connection_pool)
    .await?;

    let data: PostRequest = post_body(&request)?;

    let mut tags: Vec<String> = Vec::new();
    if let Some(tag_ids) = data.tags {
        for t in tag_ids {
            let result = query!(
                "SELECT name FROM tags WHERE id=$1 AND site_id=$2",
                t,
                globals.site_id
            )
            .fetch_one(&globals.connection_pool)
            .await?;
            tags.push(result.name);
        }
    }

    let post = HydratedPost {
        id: 0,
        post_date: data.date,
        url_slug: "preview".into(),
        title: data.title,
        body: data.body,
        song: data.song,
        mood: data.mood,
        summary: data.summary,
        author_name: user.display_name,
        comment_count: Some(0),
        tags: Some(tags),
        site_id: globals.site_id,
    };

    let post_page = PostPage {
        title: &post.title,
        post: &post,
        common: &common,
        comments: [].into(),
    };

    render_html(post_page)
}

pub async fn regenerate_blog(globals: &PageGlobals) -> anyhow::Result<cgi::Response> {
    let output_path =
        env::var("BLOG_OUTPUT_PATH").expect("Environment variable BLOG_OUTPUT_PATH is required");

    let posts = query_as!(
        HydratedPost,
        r#"
SELECT
    posts.id as id,
    post_date,
    url_slug,
    title,
    body,
    song,
    mood,
    summary,
    users.display_name AS author_name,
    (SELECT COUNT(*) FROM comments WHERE comments.post_id = posts.id) AS comment_count,
    (SELECT array_agg(t.name) FROM tags t INNER JOIN post_tag pt ON pt.tag_id = t.id WHERE pt.post_id = posts.id) AS tags,
	site_id
FROM posts
INNER JOIN users
ON users.id = posts.author_id
WHERE state = 'published'
AND posts.site_id = $1
ORDER BY post_date DESC
"#, globals.site_id
    )
    .fetch_all(&globals.connection_pool)
    .await?;

    if posts.len() == 0 {
        return Ok(redirect_response("dashboard"));
    }

    let common = get_common(&globals.connection_pool, globals.site_id).await?;

    for post in &posts {
        generate_post_page(&output_path, post, &common, &globals.connection_pool).await?;
    }

    generate_paged(
        posts.iter().collect::<Vec<&HydratedPost>>().into_iter(),
        &common,
        &output_path,
    )
    .await?;

    regenerate_month_index_pages(&output_path, &posts, &common).await?;
    regenerate_year_index_pages(&output_path, &posts, &common).await?;
    regenerate_rss_feed(&output_path, &posts, &common).await?;
    regenerate_atom_feed(&output_path, &posts, &common).await?;
    regenerate_tag_indexes(
        &globals.connection_pool,
        &output_path,
        globals.site_id,
        &posts,
        &common,
    )
    .await?;
    regenerate_pages(
        &output_path,
        globals.site_id,
        &globals.connection_pool,
        &common,
    )
    .await?;

    Ok(redirect_response("dashboard"))
}

async fn generate_paged<'a>(
    posts: IntoIter<&HydratedPost>,
    common: &CommonData,
    output_path: &String,
) -> anyhow::Result<()> {
    let total_pages = (posts.len() as f64 / 10.0).ceil() as i32;
    for (pos, chunk) in posts.chunks(10).into_iter().enumerate() {
        let path = if pos == 0 {
            String::from("index.html")
        } else {
            format!("index{}.html", pos + 1)
        };
        let mut file = File::create(format!("{}/{}", output_path, path))?;

        let page = IndexPage {
            title: &common.blog_name,
            posts: chunk.collect(),
            common: &common,
            page: pos as i32 + 1,
            total_pages,
        };

        let rendered = page.render()?;
        write!(&mut file, "{}", rendered)?;
    }
    Ok(())
}

async fn generate_post_page(
    output_path: &String,
    post: &HydratedPost,
    common: &CommonData,
    connection: &PgPool,
) -> anyhow::Result<()> {
    let comments = query_as!(HydratedComment, "SELECT author_name, post_body, created_date FROM comments WHERE post_id=$1 AND status = 'approved' ORDER BY created_date ASC", post.id).fetch_all(connection).await?;

    let month_name = Month::from_u32(post.post_date.month())
        .ok_or(anyhow!("Bad month number"))?
        .name();
    let dir = format!("{}/{}/{}", output_path, post.post_date.year(), month_name);
    let post_path = format!("{}/{}.html", &dir, post.url_slug);
    create_dir_all(&dir).await?;

    let mut file = File::create(post_path)?;
    let post_page = PostPage {
        title: &post.title,
        post,
        common: &common,
        comments,
    };

    let rendered = post_page.render()?;
    write!(&mut file, "{}", rendered)?;

    let activitypub = query!(
        r#"SELECT activity AS "activity: Json<Activity>" FROM activitypub_outbox WHERE source_post=$1"#,
        post.id
    )
    .fetch_optional(connection)
    .await?;

    if let Some(row) = activitypub {
        if let Activity::Create(create) = row.activity.as_ref() {
            if let Activity::Note(note) = create.object() {
                let json_path = format!("{}/{}.json", &dir, post.url_slug);
                let mut json_file = File::create(json_path)?;
                write!(&mut json_file, "{}", serde_json::to_string(note)?)?;
            }
        }
    }
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
        create_dir_all(&index_dir).await?;

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

async fn regenerate_pages(
    output_path: &String,
    site_id: i32,
    connection: &PgPool,
    common: &CommonData,
) -> anyhow::Result<()> {
    let pages = query!(
        "SELECT title, url_slug, body, date_updated FROM pages WHERE site_id=$1",
        site_id
    )
    .fetch_all(connection)
    .await?;

    for page in pages {
        let mut file = File::create(format!("{}/{}.html", output_path, page.url_slug))?;
        let page = Page {
            common,
            title: page.title,
            body: page.body,
            last_updated: page.date_updated,
        }
        .render()?;

        write!(&mut file, "{}", page)?;
    }
    Ok(())
}

async fn regenerate_tag_indexes(
    connection: &PgPool,
    output_path: &String,
    site_id: i32,
    posts: &Vec<HydratedPost>,
    common: &CommonData,
) -> anyhow::Result<()> {
    let all_tags = query!("SELECT name FROM tags WHERE site_id=$1", site_id)
        .fetch_all(connection)
        .await?;
    for tag_row in all_tags {
        let tag = tag_row.name;

        let tag_posts = posts
            .iter()
            .filter(|p| p.tags.clone().map(|t| t.contains(&tag)).unwrap_or(false))
            .sorted_by(|a, b| Ord::cmp(&b.post_date, &a.post_date));

        let tag_output_path = format!("{}/tags/{}/", output_path, tag.to_lowercase());
        create_dir_all(&tag_output_path).await?;
        generate_paged(tag_posts.into_iter(), common, &tag_output_path).await?;
    }
    Ok(())
}
