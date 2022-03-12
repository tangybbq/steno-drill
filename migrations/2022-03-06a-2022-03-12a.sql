-- Migrate to the lesson with a more meaningful unique constraint
-- It is valid for a lesson to contain redundant words, but there
-- should only be a single word for a given sequence number.

BEGIN;
ALTER TABLE lesson RENAME TO lessonold;
CREATE TABLE lesson (
        word TEXT NOT NULL,
        steno TEXT NOT NULL,
        listid INTEGER REFERENCES list (id) NOT NULL,
        seq INTEGER NOT NULL,
        UNIQUE (listid, seq));
INSERT INTO lesson SELECT * FROM lessonold;
DROP TABLE lessonold;
UPDATE schema SET version = '2022-03-12a';
COMMIT;
