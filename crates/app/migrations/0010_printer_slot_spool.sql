ALTER TABLE printer_slots
    ADD CONSTRAINT printer_slots_spool_id_fkey
    FOREIGN KEY (spool_id) REFERENCES spools(id) ON DELETE SET NULL;

CREATE UNIQUE INDEX printer_slots_spool_id_unique
    ON printer_slots (spool_id)
    WHERE spool_id IS NOT NULL;
