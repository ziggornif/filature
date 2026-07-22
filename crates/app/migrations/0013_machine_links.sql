CREATE TABLE machine_links (
    printer_id TEXT PRIMARY KEY REFERENCES printers(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('prusalink', 'moonraker')),
    endpoint TEXT NOT NULL,
    credential TEXT NULL
);
