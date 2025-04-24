use chrono::{offset::Utc, DateTime, NaiveDateTime};
use chrono_tz::Tz;

pub fn format_long_datetime(date_time: &DateTime<Utc>, timezone: &Tz) -> ::askama::Result<String> {
    Ok(date_time
        .with_timezone(timezone)
        .format("%C%y-%m-%d %H:%M %Z")
        .to_string())
}

pub fn format_long_date(date_time: &DateTime<Utc>, timezone: &Tz) -> ::askama::Result<String> {
    Ok(date_time
        .with_timezone(timezone)
        .format("%-d %B %C%y")
        .to_string())
}

pub fn clean_html<S>(content: S) -> ::askama::Result<String>
where
    S: AsRef<str>,
{
    Ok(ammonia::clean(content.as_ref()))
}

pub fn format_form_date(date_time: &NaiveDateTime) -> ::askama::Result<String> {
    Ok(date_time.format("%Y-%m-%d %H:%M:%S").to_string())
}
