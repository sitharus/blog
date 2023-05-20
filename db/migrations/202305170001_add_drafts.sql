CREATE TYPE post_status AS ENUM (
	   'draft',
	   'published'
);

ALTER TABLE posts ADD COLUMN state post_status NOT NULL DEFAULT 'draft';
ALTER TABLE posts RENAME COLUMN post_date TO created_date;
ALTER TABLE posts ADD COLUMN post_date DATE;

UPDATE posts SET post_date = CAST(created_date AS date), state='published';

ALTER TABLE posts ALTER COLUMN post_date SET NOT NULL;
