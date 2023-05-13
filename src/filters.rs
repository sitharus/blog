use time::{OffsetDateTime, format_description};

pub fn format_long_date(date_time: &OffsetDateTime) -> ::askama::Result<String>  {
	let format = format_description::parse("[year]-[month]-[day] [hour]:[minute]:[second]").map_err(|_| ::askama::Error::Custom("".into()))?;
	date_time.format(&format).map_err(|e| ::askama::Error::Custom(e.into()))
}
