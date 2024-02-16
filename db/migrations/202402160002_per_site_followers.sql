CREATE TABLE IF NOT EXISTS activitypub_followers (
	   site_id int not null references sites(id),
	   actor_id int not null references activitypub_known_actors(id),
	   PRIMARY KEY(site_id, actor_id)
);

INSERT INTO activitypub_followers(site_id, actor_id)
SELECT 1, id
FROM activitypub_known_actors WHERE is_following=true
ON CONFLICT DO NOTHING;
