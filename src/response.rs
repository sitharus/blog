pub fn redirect_response(destination: &str) -> cgi::Response {
    let body: Vec<u8> = "Redirecting".as_bytes().to_vec();
    http::response::Builder::new()
        .status(302)
        .header(http::header::LOCATION, format!("?action={}", destination))
        .header(http::header::CONTENT_TYPE, "text/plain")
        .body(body)
        .unwrap()
}
