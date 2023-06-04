use async_std::task;
use chrono::{DateTime, FixedOffset, Utc};
use lazy_static::lazy_static;
use regex::{Match, Regex};
use sqlx::types::ipnetwork::IpNetwork;
use sqlx::{query, Connection, PgConnection};
use std::fs::File;
use std::io::{self, BufRead};
use std::net::IpAddr;
use std::path::Path;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Options {
    #[arg(short, long)]
    source_file: String,
    #[arg(short, long)]
    connection_string: String,
}

#[derive(Debug)]
struct LogLine<'a> {
    source_ip: IpNetwork,
    identd: Option<&'a str>,
    user_id: Option<&'a str>,
    date_time: DateTime<Utc>,
    method: &'a str,
    path: &'a str,
    protocol: &'a str,
    response_code: i16,
    response_size: i32,
    referrer: Option<&'a str>,
    user_agent: Option<&'a str>,
}

fn main() {
    let args = Options::parse();
    task::block_on(parse_file(args))
}

async fn parse_file(args: Options) {
    let mut connection = PgConnection::connect(&args.connection_string)
        .await
        .unwrap();
    let lines = read_lines(args.source_file).expect("Could not open file!");
    for maybe_line in lines {
        if let Ok(line) = maybe_line {
            parse_line(line, &mut connection).await;
        }
    }
}

lazy_static! {
    static ref LOG_PARSE: Regex = Regex::new(
        r#"^(\S+) (\S+) (\S+) \[([\w:/]+\s[+\-]\d{4})\] (?:"(\S+) (\S+)\s*(\S+)?")\s* (\d{3}) (\S+) (?:"(.*)") (?:"(.*)")"#,
    )
    .expect("Could not parse regex");
}

async fn parse_line(line: String, connection: &mut PgConnection) {
    if let Some(parts) = LOG_PARSE.captures(&line) {
        let path = parts.get(6).unwrap().as_str();
        if path.ends_with(".html") {
            let datetime: DateTime<FixedOffset> =
                DateTime::parse_from_str(parts.get(4).unwrap().as_str(), "%d/%b/%Y:%H:%M:%S %z")
                    .unwrap();

            let response_code: i16 = parts.get(8).unwrap().as_str().parse().unwrap();
            let response_size: i32 = parts
                .get(9)
                .map(|f| f.as_str())
                .and_then(|s| s.parse().ok())
                .unwrap_or_default();

            let source_ip: IpAddr = parts.get(1).unwrap().as_str().parse().unwrap();

            let line = LogLine {
                source_ip: source_ip.into(),
                identd: none_if_dash(parts.get(2)),
                user_id: none_if_dash(parts.get(3)),
                date_time: datetime.into(),
                method: parts.get(5).unwrap().as_str(),
                path,
                protocol: parts.get(7).unwrap().as_str(),
                response_code,
                response_size,
                referrer: none_if_dash(parts.get(10)),
                user_agent: none_if_dash(parts.get(11)),
            };

            query!(
                r"
INSERT INTO raw_access_logs(source_ip, ident, user_id, date_time, method, path, protocol, response_code, response_size, referrer, user_agent)
VALUES($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
ON CONFLICT (source_ip, date_time, path) DO NOTHING
", line.source_ip, line.identd, line.user_id, line.date_time, line.method, line.path, line.protocol, line.response_code, line.response_size, line.referrer, line.user_agent
            ).execute(connection).await.unwrap();

            println!("{:?}", line);
        }
    }
}

fn none_if_dash(input: Option<Match>) -> Option<&str> {
    input.map(|m| m.as_str()).and_then(|s| match s {
        "-" => None,
        a => Some(a),
    })
}

fn read_lines<P>(filename: P) -> io::Result<io::Lines<io::BufReader<File>>>
where
    P: AsRef<Path>,
{
    let file = File::open(filename)?;
    Ok(io::BufReader::new(file).lines())
}
