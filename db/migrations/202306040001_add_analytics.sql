CREATE TABLE raw_access_logs (
	   source_ip inet not null,
	   ident varchar,
	   user_id varchar,
	   date_time timestamp with time zone not null,
	   method varchar not null,
	   path varchar not null,
	   protocol varchar not null,
	   response_code smallint,
	   response_size int,
	   referrer varchar,
	   user_agent varchar,
	   primary key(date_time, source_ip, path)
)
