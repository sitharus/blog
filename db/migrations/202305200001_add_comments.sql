CREATE EXTENSION IF NOT EXISTS ltree;

CREATE TYPE comment_status AS ENUM (
	   'pending',
	   'approved',
	   'spam'
);

CREATE TABLE comments (
	   id bigint primary key generated always as identity,
	   post_id int not null references posts(id) on delete cascade,
	   author_id int not null references comment_authors (id) on delete cascade,
	   created_date timestamp with time zone not null,
	   approved_date timestamp with time zone,
	   author_name varchar(200) not null,
	   author_email varchar(400) not null
	   reply_to ltree,
	   status comment_status not null default 'pending',
	   post_title varchar(400) not null,
	   post_body varchar(20000) not null
);
