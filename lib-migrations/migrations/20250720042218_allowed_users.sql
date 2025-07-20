-- Add migration script here

alter table users
    add allowed bool default false not null;
