
alter table twitter_handle_name_services
add column from_bonfida boolean not null default true,
add column from_cardinal boolean not null default false,
add column write_version bigint not null default 0;
