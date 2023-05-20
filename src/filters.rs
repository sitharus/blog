use time::{format_description, Date, OffsetDateTime};

pub fn format_long_datetime(date_time: &OffsetDateTime) -> ::askama::Result<String> {
    let format = format_description::parse("[year]-[month]-[day] [hour]:[minute]:[second]")
        .map_err(|_| ::askama::Error::Custom("".into()))?;
    date_time
        .format(&format)
        .map_err(|e| ::askama::Error::Custom(e.into()))
}

pub fn format_long_date(date_time: &Date) -> ::askama::Result<String> {
    let format = format_description::parse("[year]-[month]-[day]")
        .map_err(|_| ::askama::Error::Custom("".into()))?;
    date_time
        .format(&format)
        .map_err(|e| ::askama::Error::Custom(e.into()))
}
