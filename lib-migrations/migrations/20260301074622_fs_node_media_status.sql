-- Status of media process

alter table fs_node
    add status bool default true not null;
