CREATE TABLE activitypub_outbox (
	id bigint primary key generated always as identity,
	created_at timestamp with time zone not null default current_timestamp,
	delivery_completed_at timestamp with time zone,
	all_delivered boolean not null,
	activity_id varchar(1000) not null,
	activity jsonb not null,
	unique(activity_id)
);

CREATE TABLE activitypub_outbox_target (
	activitypub_outbox_id bigint not null references activitypub_outbox(id) on delete restrict,
	target varchar(1000) not null,
	delivered boolean not null default false,
	delivered_at timestamp with time zone,
	primary key(activitypub_outbox_id, target)
);

CREATE TABLE activitypub_delivery_log (
	id bigint primary key generated always as identity,
	activitypub_outbox_id bigint not null references activitypub_outbox(id) on delete restrict,
	target varchar(1000) not null,
	attempted_at timestamp with time zone not null default current_timestamp,
	successful boolean not null,
	status_code varchar,
	response_body VARCHAR
);
