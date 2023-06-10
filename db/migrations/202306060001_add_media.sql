CREATE TYPE media_kinds AS ENUM (
	   'image'
);

CREATE TABLE media (
	   id int primary key generated always as identity,
	   file_type media_kinds not null,
	   file varchar not null,
	   metadata jsonb
);
