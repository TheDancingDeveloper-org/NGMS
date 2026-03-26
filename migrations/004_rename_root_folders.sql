-- Rename root_folders to media_library_folders

ALTER TABLE root_folders RENAME TO media_library_folders;

ALTER TABLE series RENAME COLUMN root_folder_id TO media_library_folder_id;
ALTER TABLE movies RENAME COLUMN root_folder_id TO media_library_folder_id;
ALTER TABLE import_lists RENAME COLUMN root_folder_id TO media_library_folder_id;
