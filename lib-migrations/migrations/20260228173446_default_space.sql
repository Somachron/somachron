-- Default user space

create table default_space
(
    space_fk_id uuid not null
        constraint default_space_spaces_id_fk
            references spaces,
    user_fk_id  uuid not null
        constraint default_space_users_id_fk
            references users
);

create unique index default_space_space_fk_id_user_fk_id_uindex
    on default_space (space_fk_id, user_fk_id);
