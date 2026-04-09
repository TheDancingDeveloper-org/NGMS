-- Normalize media_files.quality to use integer IDs instead of string enum names.
-- String values like "WEBDL1080p" are converted to their integer equivalents (e.g. 11).
-- Rows already using integer IDs are left untouched.

UPDATE media_files
SET quality = jsonb_set(
    quality,
    '{quality}',
    CASE quality->>'quality'
        WHEN 'Unknown'      THEN '0'::jsonb
        WHEN 'SDTV'         THEN '1'::jsonb
        WHEN 'DVD'          THEN '2'::jsonb
        WHEN 'DVDRip'       THEN '2'::jsonb
        WHEN 'WEBDL480p'    THEN '3'::jsonb
        WHEN 'WEBRip480p'   THEN '4'::jsonb
        WHEN 'HDTV720p'     THEN '6'::jsonb
        WHEN 'WEBDL720p'    THEN '7'::jsonb
        WHEN 'WEBRip720p'   THEN '8'::jsonb
        WHEN 'Bluray720p'   THEN '9'::jsonb
        WHEN 'HDTV1080p'    THEN '10'::jsonb
        WHEN 'WEBDL1080p'   THEN '11'::jsonb
        WHEN 'WEBRip1080p'  THEN '12'::jsonb
        WHEN 'Bluray1080p'  THEN '13'::jsonb
        WHEN 'Remux1080p'   THEN '14'::jsonb
        WHEN 'HDTV2160p'    THEN '15'::jsonb
        WHEN 'WEBDL2160p'   THEN '16'::jsonb
        WHEN 'WEBRip2160p'  THEN '17'::jsonb
        WHEN 'Bluray2160p'  THEN '18'::jsonb
        WHEN 'Remux2160p'   THEN '19'::jsonb
        WHEN 'Raw'          THEN '20'::jsonb
        ELSE '0'::jsonb
    END
)
WHERE jsonb_typeof(quality->'quality') = 'string';
