-- Add-spool wizard: distinguish complete reels from holder-less refills and
-- allow the operator to leave the filament colour unset.
ALTER TABLE spools
    ADD COLUMN spool_type TEXT NOT NULL DEFAULT 'Complete'
        CHECK (spool_type IN ('Complete', 'Recharge'));

ALTER TABLE spools ALTER COLUMN colour_hex DROP NOT NULL;
