ALTER TABLE activitypub_outbox_target ADD COLUMN retries INT NOT NULL DEFAULT 0;

ALTER TABLE activitypub_outbox ADD COLUMN source_post INT REFERENCES posts(id);
ALTER TABLE activitypub_outbox ADD COLUMN is_public BOOLEAN NOT NULL DEFAULT true;
