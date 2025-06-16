use chrono::{DateTime, NaiveDateTime, offset::Utc};
use chrono_tz::Tz;

pub fn format_long_datetime(
    date_time: &DateTime<Utc>,
    _: &dyn askama::Values,
    timezone: &Tz,
) -> ::askama::Result<String> {
    Ok(date_time
        .with_timezone(timezone)
        .format("%C%y-%m-%d %H:%M %Z")
        .to_string())
}

pub fn format_long_date(
    date_time: &DateTime<Utc>,
    _: &dyn askama::Values,
    timezone: &Tz,
) -> ::askama::Result<String> {
    Ok(date_time
        .with_timezone(timezone)
        .format("%-d %B %C%y")
        .to_string())
}

pub fn clean_html<S>(content: S, _: &dyn askama::Values) -> ::askama::Result<String>
where
    S: AsRef<str>,
{
    Ok(ammonia::clean(content.as_ref()))
}

pub fn format_form_date(
    date_time: &NaiveDateTime,
    _: &dyn askama::Values,
) -> ::askama::Result<String> {
    Ok(date_time.format("%Y-%m-%d %H:%M:%S").to_string())
}

pub fn or_default(content: &Option<String>, _: &dyn askama::Values) -> ::askama::Result<String> {
    Ok(content.clone().unwrap_or("".into()))
}
