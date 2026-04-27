-- Hard cutover from fs_node/fs_link hierarchy into:
--   media_files (space-level canonical media)
--   albums (flat per-space collections)
--   album_media_files (many-to-many references)

create table media_files
(
    id            uuid                     not null
        constraint media_files_pk
            primary key,
    created_at    timestamptz              not null default now(),
    updated_at    timestamptz              not null default now(),
    user_id       uuid                     not null
        constraint media_files_users_id_fk
            references users,
    space_id      uuid                     not null
        constraint media_files_spaces_id_fk
            references spaces,
    hash          char(64)                 not null,
    file_name     varchar(255)             not null,
    object_key    varchar                  not null,
    thumbnail_key varchar,
    preview_key   varchar,
    node_size     bigint                   not null,
    metadata      jsonb                    not null default '{}'::jsonb
);

create unique index media_files_space_id_hash_uindex
    on media_files (space_id, hash);

create index media_files_space_id_updated_at_index
    on media_files (space_id, updated_at desc);

create table albums
(
    id          uuid                     not null
        constraint albums_pk
            primary key,
    created_at  timestamptz              not null default now(),
    updated_at  timestamptz              not null default now(),
    user_id     uuid                     not null
        constraint albums_users_id_fk
            references users,
    space_id    uuid                     not null
        constraint albums_spaces_id_fk
            references spaces,
    name        varchar(255)             not null,
    legacy_path varchar                  not null default ''
);

create index albums_space_id_index
    on albums (space_id);

create table album_media_files
(
    album_id      uuid        not null
        constraint album_media_files_albums_id_fk
            references albums
                on delete cascade,
    media_file_id uuid        not null
        constraint album_media_files_media_files_id_fk
            references media_files
                on delete cascade,
    created_at    timestamptz not null default now(),
    constraint album_media_files_pk
        primary key (album_id, media_file_id)
);

create index album_media_files_media_file_id_index
    on album_media_files (media_file_id);

-- 1) backfill albums (exclude synthetic root folders)
insert into albums (id, created_at, updated_at, user_id, space_id, name, legacy_path)
select id, created_at, updated_at, user_id, space_id, node_name, path
from fs_node
where node_type = 0
  and parent_node is not null;

-- 2) backfill media with per-space hash dedupe and compatibility keys
with file_candidates as (
    select
        f.id,
        f.created_at,
        f.updated_at,
        f.user_id,
        f.space_id,
        f.hash,
        f.node_name,
        f.node_size,
        f.metadata,
        nullif(trim(both '/' from f.path), '') as dir_path,
        nullif(f.metadata->'thumbnail_meta'->>'file_name', '') as thumbnail_name,
        nullif(f.metadata->'preview_meta'->>'file_name', '') as preview_name
    from fs_node f
    where f.node_type = 1
), normalized as (
    select
        id,
        created_at,
        updated_at,
        user_id,
        space_id,
        hash,
        node_name,
        node_size,
        metadata,
        case
            when dir_path is null then node_name
            else concat(dir_path, '/', node_name)
        end as object_key,
        case
            when thumbnail_name is null then null
            when dir_path is null then thumbnail_name
            else concat(dir_path, '/', thumbnail_name)
        end as thumbnail_key,
        case
            when preview_name is null then null
            when dir_path is null then preview_name
            else concat(dir_path, '/', preview_name)
        end as preview_key
    from file_candidates
), ranked as (
    select
        n.*,
        row_number() over (
            partition by n.space_id, n.hash
            order by
                ((n.thumbnail_key is not null)::int + (n.preview_key is not null)::int) desc,
                n.created_at asc,
                n.id asc
        ) as rn
    from normalized n
)
insert into media_files
(
    id,
    created_at,
    updated_at,
    user_id,
    space_id,
    hash,
    file_name,
    object_key,
    thumbnail_key,
    preview_key,
    node_size,
    metadata
)
select
    id,
    created_at,
    updated_at,
    user_id,
    space_id,
    hash,
    node_name,
    object_key,
    thumbnail_key,
    preview_key,
    node_size,
    metadata
from ranked
where rn = 1;

-- 3) backfill album -> canonical media references based on fs_link
insert into album_media_files (album_id, media_file_id)
select distinct
    a.id,
    mf.id
from fs_link l
inner join fs_node folder
    on folder.id = l.node_id
    and folder.node_type = 0
    and folder.parent_node is not null
inner join albums a
    on a.id = folder.id
inner join fs_node file_node
    on file_node.id = l.child_node_id
    and file_node.node_type = 1
inner join media_files mf
    on mf.space_id = file_node.space_id
    and mf.hash = file_node.hash
on conflict do nothing;

-- 4) validate backfill invariants before dropping legacy tables
DO $$
declare
    expected_album_count bigint;
    actual_album_count bigint;
    expected_media_count bigint;
    actual_media_count bigint;
begin
    select count(*) into expected_album_count
    from fs_node
    where node_type = 0
      and parent_node is not null;

    select count(*) into actual_album_count
    from albums;

    if expected_album_count <> actual_album_count then
        raise exception 'Album backfill mismatch: expected %, got %', expected_album_count, actual_album_count;
    end if;

    select count(distinct (space_id, hash)) into expected_media_count
    from fs_node
    where node_type = 1;

    select count(*) into actual_media_count
    from media_files;

    if expected_media_count <> actual_media_count then
        raise exception 'Media backfill mismatch: expected %, got %', expected_media_count, actual_media_count;
    end if;
end;
$$;

-- 5) final hard cutover
DROP TABLE fs_link;
DROP TABLE fs_node;
