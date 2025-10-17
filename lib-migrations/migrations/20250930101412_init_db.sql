-- USERS

create table users
(
    id          uuid                     default gen_random_uuid() not null
        constraint user_pk
            primary key,
    created_at  timestamp with time zone default now()             not null,
    updated_at  timestamp with time zone default now()             not null,
    allowed     boolean                  default false             not null,
    clerk_id    char(32)                                           not null,
    email       varchar                                            not null,
    first_name  varchar                                            not null,
    last_name   varchar                                            not null,
    picture_url varchar                                            not null
);

create unique index users_clerk_id_uindex
    on users (clerk_id);

-- SPACE

create table spaces
(
    id          uuid                     default gen_random_uuid() not null
        constraint spaces_pk
            primary key,
    created_at  timestamp with time zone default now()             not null,
    updated_at  timestamp with time zone default now()             not null,
    name        varchar                                            not null,
    description varchar                                            not null,
    picture_url varchar                                            not null
);

-- USERSPACE

create table users_spaces
(
    id         uuid        default gen_random_uuid() not null
        constraint users_spaces_pk
            primary key,
    created_at timestamptz default now()             not null,
    updated_at timestamptz default now()             not null,
    user_id    uuid                                  not null
        constraint users_spaces_users_id_fk
            references users,
    space_id   uuid                                  not null
        constraint users_spaces_spaces_id_fk
            references spaces,
    role       smallint                              not null
);

-- FS NODE

create table fs_node
(
    id          uuid        default gen_random_uuid() not null
        constraint fs_node_pk
            primary key,
    created_at  timestamptz default now()             not null,
    updated_at  timestamptz default now()             not null,
    user_id     uuid
        constraint fs_node_user_id_fk
            references users,
    space_id    uuid                                  not null
        constraint fs_node_space_id_fk
            references spaces,
    node_type   smallint                              not null,
    node_size   bigint                                not null,
    parent_node uuid
        constraint fs_node_fs_node_id_fk
            references fs_node,
    node_name   varchar(255)                          not null,
    path        varchar                               not null,
    metadata    jsonb       default '{}'::jsonb       not null
);

create index fs_node_node_type_index
    on fs_node (node_type);

create unique index fs_node_node_name_space_id_parent_node_uindex
    on fs_node (node_name, space_id, parent_node)
    nulls not distinct;

-- FS LINK

create table fs_link
(
    node_id       uuid not null
        constraint fs_link_fs_node_id_fk
            references fs_node,
    child_node_id uuid not null
        constraint fs_link_fs_node_id_fk_child
            references fs_node,
    constraint fs_link_pk
        primary key (node_id, child_node_id)
);

create unique index fs_link_child_node_id_uindex
    on fs_link (child_node_id);
