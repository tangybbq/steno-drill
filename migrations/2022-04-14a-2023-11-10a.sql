-- The learn table has a factor associated with each word. Any time an error is
-- made, this factor is reduced by an amount, which will lower the amount of
-- increase that comes from each "good".

BEGIN;
ALTER TABLE learn RENAME to learnold;
CREATE TABLE learn (
  word TEXT UNIQUE PRIMARY KEY,
  steno TEXT NOT NULL,
  goods INTEGER NOT NULL,
  interval REAL NOT NULL,
  factor REAL NOT NULL,
  next REAL NOT NULL);
INSERT INTO learn SELECT word, steno, goods, interval, 4.0, next FROM learnold WHERE true;
DROP TABLE learnold;
UPDATE schema SET version = '2023-11-10a';
COMMIT;
