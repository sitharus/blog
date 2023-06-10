# Blog!

This is the backend to [my blog](https://thea.hutchings.gen.nz/). It
runs as a CGI application because that's easy, it generates static
HTML so there's no per-request overhead, and it supports comments.

This _is not_ an example of how to build a public use site, it lacks:

 - Proper error handling/feedback. Most errors result in a 500 with no detail.
 - Input sanitisation - if I can't trust me I have problems
 - Any indication that you forgot to click regenerate

But it has:

 - Static page rendering
 - Atom/RSS feeds
 - Markdown based text formatting
 - Sidebar links
 - Archive index pages
 - Comments
 - Image uploads
 - Non-blog pages
 - Absolutely no Javascript (this may change in the admin console)

And I plan on

 - Being timezone aware (currently everything is UTC)
 - Server log based analytics
 - Better image management (multiple formats/sizes for optimal display)
 - Maybe trackbacks?

# Technology

Rust
 - sqlx for DB queries
 - askama for templating
 - chrono for time
 - async-std because async

HTML/CSS for display

PostgreSQL for data

# Building (so I remember)

First build the migrations - building the rest requires a DB connection
`cargo build -p migrations`

Create your postgres database and load `initial.sql`, then migrate.

`BLOG_CONNECTION_STRING=postgres://blog:blog@localhost/blog ./target/debug/migrations`

and add `DATABASE_URL='postgres://blog:blog@localhost/blog'` to .env

Now build the rest with `cargo build -r`

Copy `static` to your webserver root, copy `target/release/admin` to `cgi-bin/admin.cgi` (or whereever you put your CGIs, the name doesn't matter) and `target/release/public` to `cgi-bin/public.cgi` (again name doesn't matter, you can configure it.

Configure your web server to expose these environment variables:

```
BLOG_CONNECTION_STRING="postgres://blog:blog@localhost/blog"
BLOG_OUTPUT_PATH="/path/to/web/root"
```
