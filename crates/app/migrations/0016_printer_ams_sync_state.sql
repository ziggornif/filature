ALTER TABLE printers
    ADD COLUMN ams_sync_state TEXT NOT NULL DEFAULT 'offline',
    ADD CONSTRAINT printers_ams_sync_state_check
        CHECK (ams_sync_state IN ('up_to_date', 'drift', 'offline'));
