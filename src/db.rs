//! Learning database operations.

use crate::Lesson;
use anyhow::{bail, Result};
use rusqlite::{named_params, Connection};
use std::path::Path;

/// The schema version that matches this code.  May be usable in the future for automatic upgrades.
static SCHEMA_VERSION: &'static str = "2022-03-02a";

static SCHEMA: &'static [&'static str] = &[
    "CREATE TABLE learn (
        word TEXT UNIQUE PRIMARY KEY,
        steno TEXT NOT NULL,
        goods INTEGER NOT NULL,
        interval REAL NOT NULL,
        next REAL NOT NULL);",
    "CREATE INDEX learn_steno_idx ON learn (steno);",
    "CREATE INDEX learn_next_idx ON learn (next);",
    "CREATE TABLE list (
        id INTEGER PRIMARY KEY,
        name TEXT UNIQUE NOT NULL);",
    "CREATE TABLE lesson (
        word TEXT NOT NULL,
        steno TEXT NOT NULL,
        listid INTEGER REFERENCES list (id) NOT NULL,
        seq INTEGER NOT NULL,
        UNIQUE (word, listid));",
    "CREATE TABLE schema (version TEXT NOT NULL);",
];

pub struct Db {
    conn: Connection,
}

impl Db {
    /// Initialize a new database.  The file shouldn't exist, and will likely generate an error if
    /// it does.
    pub fn init<P: AsRef<Path>>(path: P) -> Result<()> {
        let mut conn = Connection::open(path)?;
        let tx = conn.transaction()?;

        for line in SCHEMA {
            tx.execute(line, [])?;
        }
        tx.execute("INSERT INTO schema (version) VALUES (:version)", &[(":version", SCHEMA_VERSION)])?;
        tx.commit()?;
        Ok(())
    }

    /// Open the database
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Db> {
        let conn = Connection::open(path)?;
        let version: String =
            conn.query_row("SELECT version FROM schema", [],
                |row| row.get(0))?;
        if version != SCHEMA_VERSION {
            bail!("Schema version mismatch: found {}, want {}", version, SCHEMA_VERSION);
        }

        Ok(Db { conn })
    }

    /// Load the words from the given lesson into the database.
    pub fn load(&mut self, lesson: &Lesson) -> Result<()> {
        let tx = self.conn.transaction()?;

        // Create the lesson, getting its new ID.
        tx.execute("INSERT INTO list (name) VALUES (:name)",
            &[(":name", &lesson.description)])?;
        let id = tx.last_insert_rowid();
        println!("New ID: {}", id);

        for (seq, entry) in lesson.entries.iter().enumerate() {
            let steno = format!("{}", entry.steno);
            // println!("entry: {} {}", entry.word, entry.steno);
            match tx.execute("INSERT INTO lesson (word, steno, listid, seq)
                VALUES (:word, :steno, :listid, :seq)",
                named_params! {
                    ":word": &entry.word,
                    ":steno": &steno,
                    ":listid": id,
                    ":seq": seq + 1,
                }) {
                Ok(_) => (),
                Err(msg) => {
                    println!("Warn: {}", msg);
                }
            }
        }

        tx.commit()?;

        Ok(())
    }

    /// Show the information about lessons.
    pub fn info(&mut self, seen: bool, unseen: bool) -> Result<()> {
        let mut stmt = self.conn.prepare("SELECT
            list.id,
            (SELECT COUNT(*) FROM learn, lesson WHERE
                lesson.listid = list.id AND
                learn.word = lesson.word),
            (SELECT COUNT(*) FROM lesson WHERE lesson.listid = list.id),
            name
            FROM list
            ORDER by list.id")?;
        for row in stmt.query_map([], |row|
            Ok(InfoResult {
                id: row.get(0)?,
                num: row.get(1)?,
                total: row.get(2)?,
                name: row.get(3)?,
            }))?
        {
            let row = row?;
            if seen && row.num == 0 {
                continue;
            }
            if unseen && row.num > 0 {
                continue;
            }
            println!("  {:2}. {:5}/{:<5}: {}", row.id, row.num, row.total, row.name);
        }

        Ok(())
    }
}

struct InfoResult {
    id: i64,
    num: usize,
    total: usize,
    name: String,
}
