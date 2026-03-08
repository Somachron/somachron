-- FS Node .. add media hash sha256

alter table fs_node
    drop column status;

-- add hash

alter table fs_node
    add hash char(64);

update fs_node set hash = encode(sha256(id::text::bytea), 'hex');

alter table fs_node
    alter column hash set not null;
