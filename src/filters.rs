use chrono::{offset::Utc, DateTime, Datelike, NaiveDate};

pub fn format_long_datetime(date_time: &DateTime<Utc>) -> ::askama::Result<String> {
    Ok(date_time.format("%Y-%M-%d %H:%m:%s").to_string())
}

pub fn format_long_date(date_time: &NaiveDate) -> ::askama::Result<String> {
    Ok(format!(
        "{}-{}-{}",
        date_time.year(),
        date_time.month(),
        date_time.day()
    ))
}
