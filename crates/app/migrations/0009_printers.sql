CREATE TABLE printers (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    brand TEXT NOT NULL,
    model TEXT NOT NULL,
    module_kind TEXT NOT NULL,
    module_count INTEGER NULL
);

CREATE TABLE printer_slots (
    id TEXT PRIMARY KEY,
    printer_id TEXT NOT NULL REFERENCES printers(id) ON DELETE CASCADE,
    group_label TEXT NOT NULL,
    slot_key TEXT NOT NULL,
    position INTEGER NOT NULL,
    spool_id TEXT NULL,
    UNIQUE(printer_id, slot_key)
);
