-- Locations slice — where spools are physically stored (ADR-0003, PostgreSQL).
-- `id` is a client-generated ULID, same convention as `materials`/`spools`.
CREATE TABLE locations (
    id   TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    note TEXT NULL
);

ALTER TABLE spools ADD COLUMN location_id TEXT NULL REFERENCES locations(id);

CREATE INDEX idx_spools_location_id ON spools (location_id);
