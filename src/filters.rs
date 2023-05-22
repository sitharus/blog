use chrono::{offset::Utc, DateTime, NaiveDate};

pub fn format_long_datetime(date_time: &DateTime<Utc>) -> ::askama::Result<String> {
    Ok(date_time.format("%C%y-%M-%d %H:%m").to_string())
}

pub fn format_long_date(date_time: &NaiveDate) -> ::askama::Result<String> {
    Ok(date_time.format("%-d %B %C%y").to_string())
}
