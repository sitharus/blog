CREATE TABLE IF NOT EXISTS bsky_outbox (
	   id bigint not null primary key generated always as identity,
	   post_id bigint not null references posts(id),
	   posted_at timestamp with time zone
);
