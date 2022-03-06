-- This schema is only different because of this new table.
CREATE TABLE history (
	entry TEXT NOT NULL,
	start DATETIME NOT NULL,
	stop DATETIME);
UPDATE schema SET version = '2022-03-06a';
