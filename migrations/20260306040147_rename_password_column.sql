-- migratons/20260306040147_rename_password_column
ALTER TABLE users RENAME COLUMN password TO password_hash;
