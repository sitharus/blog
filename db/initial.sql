CREATE TABLE users (
	   id int primary key generated always as identity,
	   username varchar(250) not null,
	   password varchar(100) not null,
	   unique(username)
);

CREATE TABLE session (
	   id uuid primary key,
	   user_id int not null references users(id) on delete cascade,
	   expiry timestamp with time zone not null
);

CREATE TABLE posts (
	   id int primary key generated always as identity,
	   author_id int not null references users(id) on delete restrict,
	   post_date timestamp with time zone not null,
	   updated_date timestamp with time zone not null,
	   url_slug varchar(80) not null,
	   title varchar(500) not null,
	   body text not null,
	   unique(url_slug)
);


CREATE TABLE migrations (
	   name varchar primary key,
	   date_applied timestamp with time zone not null
);
