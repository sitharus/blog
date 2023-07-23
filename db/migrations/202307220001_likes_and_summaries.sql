CREATE TABLE activitypub_likes (
	   post_id INT NOT NULL REFERENCES posts(id) ON DELETE CASCADE,
	   inbox_item_id BIGINT NOT NULL REFERENCES activitypub_INBOX(id) ON DELETE CASCADE,
	   actor_id BIGINT NOT NULL REFERENCES activitypub_known_actors(id) ON DELETE CASCADE,
	   PRIMARY KEY(post_id, actor_id)
);

ALTER TABLE posts ADD COLUMN summary TEXT;
