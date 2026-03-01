-- User id column cannot be null

alter table fs_node
    alter column user_id set not null;
