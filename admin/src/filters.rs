use chrono::{offset::Utc, DateTime, NaiveDate};
use chrono_tz::Tz;

pub fn format_long_datetime(date_time: &DateTime<Utc>, timezone: &Tz) -> ::askama::Result<String> {
    Ok(date_time
        .with_timezone(timezone)
        .format("%C%y-%m-%d %H:%M %Z")
        .to_string())
}

pub fn format_long_date(date_time: &NaiveDate) -> ::askama::Result<String> {
    Ok(date_time.format("%-d %B %C%y").to_string())
}
