ALTER TABLE printers
ADD COLUMN heads INTEGER NOT NULL DEFAULT 1 CHECK (heads >= 1);

UPDATE printers
SET heads = COALESCE(module_count, 1),
    module_kind = 'none',
    module_count = NULL
WHERE module_kind = 'tool_changer';

UPDATE printers
SET module_kind = 'multi_slot'
WHERE module_kind IN ('indx', 'multi_colour');
