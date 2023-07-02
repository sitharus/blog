CREATE TABLE activitypub_feed (
	   id BIGINT PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
	   actor_id BIGINT NOT NULL REFERENCES activitypub_known_actors(id) ON DELETE RESTRICT,
	   inbox_item_id BIGINT NOT NULL REFERENCES activitypub_inbox(id) ON DELETE RESTRICT,
	   recieved_at TIMESTAMP WITH TIME ZONE NOT NULL,
	   message_timestamp TIMESTAMP WITH TIME ZONE NOT NULL,
	   message text,
	   extra_data jsonb
);

CREATE INDEX ix_message_timestamp ON activitypub_feed(message_timestamp);
