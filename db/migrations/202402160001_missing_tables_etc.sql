ALTER TABLE users ADD COLUMN IF NOT EXISTS display_name text;

CREATE TABLE IF NOT EXISTS external_links (
	   id int not null primary key generated always as identity,
	   title text not null,
	   destination text not null,
	   position int not null
);

ALTER TABLE external_links ADD COLUMN IF NOT EXISTS site_id int NOT NULL references sites(id) DEFAULT 1;
ALTER TABLE external_links ALTER COLUMN site_id DROP DEFAULT;

ALTER TABLE blog_settings ALTER COLUMN value SET NOT NULL;
