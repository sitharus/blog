ALTER TABLE activitypub_known_actors ADD COLUMN inbox VARCHAR(1000) NOT NULL;
ALTER TABLE activitypub_known_actors ADD COLUMN public_key_id VARCHAR(1000) NOT NULL;
