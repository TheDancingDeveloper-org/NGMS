-- Rename root_folders to media_library_folders
-- (no-op: migration 001 already uses media_library_folders)

DO $$ BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'root_folders') THEN
        ALTER TABLE root_folders RENAME TO media_library_folders;
    END IF;
END $$;

DO $$ BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'series' AND column_name = 'root_folder_id') THEN
        ALTER TABLE series RENAME COLUMN root_folder_id TO media_library_folder_id;
    END IF;
END $$;

DO $$ BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'movies' AND column_name = 'root_folder_id') THEN
        ALTER TABLE movies RENAME COLUMN root_folder_id TO media_library_folder_id;
    END IF;
END $$;

DO $$ BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'import_lists' AND column_name = 'root_folder_id') THEN
        ALTER TABLE import_lists RENAME COLUMN root_folder_id TO media_library_folder_id;
    END IF;
END $$;
