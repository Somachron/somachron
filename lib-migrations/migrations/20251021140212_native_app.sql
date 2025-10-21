-- native_app

create table native_app
(
    id                uuid                     default gen_random_uuid() not null
        constraint native_app_pk
            primary key,
    created_at        timestamp with time zone default now()             not null,
    updated_at        timestamp with time zone default now()             not null,
    name              varchar                                            not null,
    secure_identifier varchar                                            not null
);
