-- Add migration script here

create table users
(
    id          char(8)                                   not null
        constraint users_pk
            primary key,
    created_at  timestamp with time zone default now()    not null,
    updated_at  timestamp with time zone default now()    not null,
    given_name  text                                      not null,
    email       text                                      not null,
    picture_url text                     default ''::text not null
);

create unique index users_email_uindex
    on users (email);

create table spaces
(
    id          char(8)                                   not null
        constraint spaces_pk
            primary key,
    created_at  timestamp with time zone default now()    not null,
    updated_at  timestamp with time zone default now()    not null,
    name        text                                      not null,
    description text                     default ''::text not null,
    picture_url text                     default ''::text not null
);

-- owner
-- read
-- upload (can only delete their uploaded media)
-- modify (can delete anyone's uploaded media | can invite people)

create type space_role as ENUM ('owner', 'read', 'upload', 'modify');

create table users_spaces
(
    id         char(8)                                not null
        constraint users_spaces_pk
            primary key,
    created_at timestamp with time zone default now() not null,
    updated_at timestamp with time zone default now() not null,
    user_id    char(8)                                not null
        constraint users_spaces_users_id_fk
            references users,
    space_id   char(8)                                not null
        constraint users_spaces_spaces_id_fk
            references spaces,
    role       space_role                             not null
);
