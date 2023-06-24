CREATE TABLE activitypub_inbox (
	   id bigint primary key generated always as identity,
	   received_at timestamp with time zone default current_timestamp,
	   body jsonb
);

CREATE TYPE activitypub_block_target AS ENUM (
	  'server',
	  'actor'
);

CREATE TABLE activitypub_blocked (
	   id bigint primary key generated always as identity,
	   target_type activitypub_block_target NOT NULL,
	   target varchar(1000),
	   unique(target_type, target)
);

CREATE TABLE activitypub_known_actors (
	   id bigint primary key generated always as identity,
	   first_seen timestamp with time zone default current_timestamp,
	   last_seen timestamp with time zone default current_timestamp,
	   is_following boolean not null,
	   actor varchar(1000),
	   public_key text,
	   unique(actor)
);
