-- New table is all.
CREATE TABLE errors (
        stamp DATETIME NOT NULL,
        word TEXT REFERENCES learn (word) NOT NULL,
        goods INTEGER NOT NULL,
        interval REAL NOT NULL,
        next REAL NOT NULL,
        actual TEXT NOT NULL);
UPDATE schema SET version = '2022-04-14a';
