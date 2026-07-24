ALTER TABLE machine_links DROP CONSTRAINT machine_links_kind_check;
ALTER TABLE machine_links
    ADD CONSTRAINT machine_links_kind_check
    CHECK (kind IN ('prusalink', 'moonraker', 'bambu'));
