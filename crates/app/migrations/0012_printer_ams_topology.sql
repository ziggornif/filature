CREATE TABLE printer_ams_units (
    printer_id TEXT NOT NULL REFERENCES printers(id) ON DELETE CASCADE,
    unit_index INTEGER NOT NULL CHECK (unit_index BETWEEN 0 AND 3),
    PRIMARY KEY (printer_id, unit_index)
);

CREATE TABLE printer_head_feed_modes (
    printer_id TEXT NOT NULL REFERENCES printers(id) ON DELETE CASCADE,
    head_index INTEGER NOT NULL CHECK (head_index >= 0),
    feed_mode TEXT NOT NULL CHECK (feed_mode IN ('direct', 'ams_fed')),
    PRIMARY KEY (printer_id, head_index)
);

INSERT INTO printer_ams_units (printer_id, unit_index)
SELECT id, 0 FROM printers WHERE brand = 'bambu' AND module_kind = 'ams';

INSERT INTO printer_head_feed_modes (printer_id, head_index, feed_mode)
SELECT p.id, head_index,
       CASE WHEN p.brand = 'bambu' AND p.module_kind = 'ams' THEN 'ams_fed' ELSE 'direct' END
FROM printers p
CROSS JOIN LATERAL generate_series(0, p.heads - 1) AS heads(head_index);

UPDATE printer_slots ps
SET slot_key = 'ams0-' || substring(ps.slot_key FROM 5),
    group_label = 'ams_unit_1'
FROM printers p
WHERE ps.printer_id = p.id
  AND p.brand = 'bambu'
  AND p.module_kind = 'ams'
  AND ps.slot_key LIKE 'ams-%';

DELETE FROM printer_slots ps
USING printers p
WHERE ps.printer_id = p.id AND p.brand = 'bambu' AND p.module_kind = 'ams' AND ps.slot_key = 'ext';

UPDATE printer_slots ps
SET slot_key = 'head-0', group_label = 'heads'
FROM printers p
WHERE ps.printer_id = p.id AND p.brand = 'bambu' AND p.module_kind = 'none' AND ps.slot_key = 'ext';

UPDATE printers SET module_kind = 'none', module_count = NULL
WHERE brand = 'bambu' AND module_kind = 'ams';
