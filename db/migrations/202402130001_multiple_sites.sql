CREATE TABLE IF NOT EXISTS sites (
	   id int primary key generated always as identity,
	   site_name varchar not null,

	   unique(site_name)
);

INSERT INTO sites(id, site_name) OVERRIDING SYSTEM VALUE VALUES(1, 'default site');

ALTER TABLE posts ADD COLUMN IF NOT EXISTS site_id int not null references sites(id) default 1;
ALTER TABLE posts ALTER COLUMN site_id DROP DEFAULT;

ALTER TABLE pages ADD COLUMN IF NOT EXISTS site_id int not null references sites(id) default 1;
ALTER TABLE pages ALTER COLUMN site_id DROP DEFAULT;

ALTER TABLE media ADD COLUMN IF NOT EXISTS site_id int not null references sites(id) default 1;
ALTER TABLE media ALTER COLUMN site_id DROP DEFAULT;

ALTER TABLE tags ADD COLUMN IF NOT EXISTS site_id int not null references sites(id) default 1;
ALTER TABLE tags ALTER COLUMN site_id DROP DEFAULT;

ALTER TABLE activitypub_outbox ADD COLUMN IF NOT EXISTS site_id int not null references sites(id) default 1;
ALTER TABLE activitypub_outbox ALTER COLUMN site_id DROP DEFAULT;

ALTER TABLE activitypub_feed ADD COLUMN IF NOT EXISTS site_id int not null references sites(id) default 1;
ALTER TABLE activitypub_feed ALTER COLUMN site_id DROP DEFAULT;

CREATE TABLE IF NOT EXISTS blog_settings (
	   setting_name varchar(255) not null primary key,
	   value text
);

ALTER TABLE blog_settings DROP CONSTRAINT blog_settings_pkey;
ALTER TABLE blog_settings ADD COLUMN IF NOT EXISTS site_id int not null references sites(id) default 1;
ALTER TABLE blog_settings ADD CONSTRAINT blog_settings_pkey PRIMARY KEY (site_id, setting_name);
