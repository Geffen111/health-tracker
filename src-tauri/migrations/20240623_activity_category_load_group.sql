-- Make a category's load group an explicit, editable attribute.
--
-- Until now, whether a category counted as physical, cognitive or sensory/social load was
-- inferred from substrings of its NAME, in two places that had to be kept in step by hand
-- (BUCKET_EXPR in commands/pacing.rs and computeDayLoad in src/lib/load.ts). That made the
-- mapping invisible and fragile: renaming "Domestic" would silently move it to sensory, and
-- any category the user added landed in sensory by default with no way to say otherwise.
--
-- load_group is now stored per category and edited on the Activity page, alongside
-- energy_weight. The backfill below reproduces exactly what the name rules produced, so no
-- historical figure changes.

ALTER TABLE activity_categories ADD COLUMN load_group TEXT NOT NULL DEFAULT 'sensory';

UPDATE activity_categories SET load_group =
    CASE
        WHEN LOWER(name) LIKE '%physical%' OR LOWER(name) LIKE '%domestic%' THEN 'physical'
        WHEN LOWER(name) LIKE '%cognitive%' OR LOWER(name) LIKE '%hobby%'   THEN 'cognitive'
        ELSE 'sensory'
    END;
