-- Manufacturers slice — filament brands (ADR-0003, PostgreSQL).
-- `id` is a client-generated ULID, same convention as materials/spools/locations.
-- `name` is UNIQUE: the seed referential and the "block delete when in use"
-- rule both rely on a brand appearing at most once.
CREATE TABLE manufacturers (
    id      TEXT PRIMARY KEY,
    name    TEXT NOT NULL UNIQUE,
    country TEXT NULL
);

-- A spool is optionally attributed to a manufacturer (existing spools have
-- none). Deletion of a referenced manufacturer is blocked in the domain
-- (RESTRICT would also reject it at the DB, but the domain guard gives a
-- friendly count-bearing error before we get there).
ALTER TABLE spools ADD COLUMN manufacturer_id TEXT NULL REFERENCES manufacturers(id);

CREATE INDEX idx_spools_manufacturer_id ON spools (manufacturer_id);
