-- Spools slice — physical filament spools (ADR-0003, PostgreSQL).
-- `id` is a client-generated ULID, same convention as `materials`.
-- `created_at` is DB-side only (the domain aggregate has no such field);
-- it exists solely to support `ORDER BY created_at` in list sorting.
CREATE TABLE spools (
    id               TEXT PRIMARY KEY,
    material_id      TEXT NOT NULL REFERENCES materials(id),
    colour_hex       TEXT NOT NULL,
    colour_name      TEXT,
    diameter         TEXT NOT NULL CHECK (diameter IN ('1.75','2.85')),
    net_weight       DOUBLE PRECISION NOT NULL CHECK (net_weight > 0),
    remaining_weight DOUBLE PRECISION NOT NULL CHECK (remaining_weight >= 0),
    price_paid       NUMERIC NOT NULL,
    status           TEXT NOT NULL CHECK (status IN ('Sealed','Open','Empty','Archived')),
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_spools_material_id ON spools (material_id);
CREATE INDEX idx_spools_status ON spools (status);
