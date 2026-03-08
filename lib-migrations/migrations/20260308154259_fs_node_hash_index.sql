-- Index the media hash

create index fs_node_hash_index
    on fs_node (hash);
