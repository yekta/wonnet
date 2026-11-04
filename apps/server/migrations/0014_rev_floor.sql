-- An app whose cursor is below the floor gets every change again. The database came back from a
-- backup on 2026-11-04 with revs behind what the apps had seen, so they skipped what came after;
-- the jump puts every rev given out before it below the floor.
SELECT setval('revs', (SELECT last_value FROM revs) + 1000000000);
CREATE TABLE rev_floor (rev BIGINT NOT NULL);
INSERT INTO rev_floor SELECT last_value FROM revs;
