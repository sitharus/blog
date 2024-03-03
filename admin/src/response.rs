pub fn redirect_response(destination: &str) -> cgi::Response {
    let body: Vec<u8> = "Redirecting".as_bytes().to_vec();
    http::response::Builder::new()
        .status(302)
        .header(http::header::LOCATION, format!("?action={}", destination))
        .header(http::header::CONTENT_TYPE, "text/plain")
        .body(body)
        .unwrap()
}

pub fn css_response(content: &str) -> anyhow::Result<cgi::Response> {
    http::response::Builder::new()
        .status(200)
        .header(http::header::CONTENT_TYPE, "text/css")
        .body(content.as_bytes().to_vec())
        .map_err(|e| anyhow::anyhow!(e))
}

pub fn font_response(content: &[u8]) -> anyhow::Result<cgi::Response> {
    http::response::Builder::new()
        .status(200)
        .header(http::header::CONTENT_TYPE, "font/woff2")
        .body(content.into())
        .map_err(|e| anyhow::anyhow!(e))
}
