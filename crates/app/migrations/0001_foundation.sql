-- Foundation migration. Domain tables are added by their slices
-- (materials, spools, locations) as later numbered migrations.
-- This file establishes the migrations table + ordering baseline.
PRAGMA foreign_keys = ON;
