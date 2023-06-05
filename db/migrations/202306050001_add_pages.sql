CREATE TABLE pages (
	   id int primary key generated always as identity,
	   author_id int not null references users(id) on delete restrict,
	   date_updated timestamp with time zone not null,
	   url_slug varchar(80) not null,
	   title varchar(500) not null,
	   body text not null,
	   unique(url_slug)
);
