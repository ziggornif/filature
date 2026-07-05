-- Materials slice — filament catalog (ADR-0003, PostgreSQL).
-- `id` is a client-generated ULID (26-char, lexicographically sortable) so
-- insertion order is preserved without a DB-side sequence.
CREATE TABLE materials (
    id            TEXT PRIMARY KEY,
    name          TEXT NOT NULL UNIQUE,
    density       DOUBLE PRECISION NOT NULL CHECK (density > 0),
    drying_temp_c INTEGER NOT NULL,
    drying_time_h INTEGER NOT NULL,
    sensitivity   TEXT NOT NULL CHECK (sensitivity IN ('Low','Medium','High')),
    nozzle_c      INTEGER NOT NULL,
    bed_c         INTEGER NOT NULL
);
