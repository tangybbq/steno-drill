//! Learning database operations.

use crate::stroke::StenoPhrase;
use crate::Lesson;
use anyhow::{bail, Result};
use rusqlite::{named_params, Connection, OptionalExtension};
use std::path::Path;
use std::time::SystemTime;

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
        tx.execute(
            "INSERT INTO schema (version) VALUES (:version)",
            &[(":version", SCHEMA_VERSION)],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Open the database
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Db> {
        let conn = Connection::open(path)?;
        let version: String = conn.query_row("SELECT version FROM schema", [], |row| row.get(0))?;
        if version != SCHEMA_VERSION {
            bail!(
                "Schema version mismatch: found {}, want {}",
                version,
                SCHEMA_VERSION
            );
        }

        Ok(Db { conn })
    }

    /// Load the words from the given lesson into the database.
    pub fn load(&mut self, lesson: &Lesson) -> Result<()> {
        let tx = self.conn.transaction()?;

        // Create the lesson, getting its new ID.
        tx.execute(
            "INSERT INTO list (name) VALUES (:name)",
            &[(":name", &lesson.description)],
        )?;
        let id = tx.last_insert_rowid();
        println!("New ID: {}", id);

        for (seq, entry) in lesson.entries.iter().enumerate() {
            let steno = format!("{}", entry.steno);
            // println!("entry: {} {}", entry.word, entry.steno);
            match tx.execute(
                "INSERT INTO lesson (word, steno, listid, seq)
                VALUES (:word, :steno, :listid, :seq)",
                named_params! {
                    ":word": &entry.word,
                    ":steno": &steno,
                    ":listid": id,
                    ":seq": seq + 1,
                },
            ) {
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
        let mut stmt = self.conn.prepare(
            "SELECT
            list.id,
            (SELECT COUNT(*) FROM learn, lesson WHERE
                lesson.listid = list.id AND
                learn.word = lesson.word),
            (SELECT COUNT(*) FROM lesson WHERE lesson.listid = list.id),
            name
            FROM list
            ORDER by list.id",
        )?;
        for row in stmt.query_map([], |row| {
            Ok(InfoResult {
                id: row.get(0)?,
                num: row.get(1)?,
                total: row.get(2)?,
                name: row.get(3)?,
            })
        })? {
            let row = row?;
            if seen && row.num == 0 {
                continue;
            }
            if unseen && row.num > 0 {
                continue;
            }
            println!(
                "  {:2}. {:5}/{:<5}:{} {}",
                row.id,
                row.num,
                row.total,
                if row.num == row.total { '✓' } else { ' ' },
                row.name
            );
        }

        Ok(())
    }

    /// Query some words that need to be learned, returning up to count of them.
    pub fn get_drills(&mut self, count: usize) -> Result<Vec<Work>> {
        let now = get_now();
        let mut result = vec![];

        let mut stmt = self.conn.prepare(
            "
            SELECT word, steno, goods, interval, next
            FROM learn
            WHERE next < :now
            ORDER BY next
            LIMIT :limit",
        )?;
        for row in stmt.query_map(
            named_params! {
                ":now": now,
                ":limit": count,
            },
            |row| {
                let steno: String = row.get(1)?;
                Ok(Work {
                    text: row.get(0)?,
                    strokes: StenoPhrase::parse(&steno).unwrap(),
                    goods: row.get(2)?,
                    interval: row.get(3)?,
                    next: row.get(4)?,
                })
            },
        )? {
            result.push(row?);
        }

        Ok(result)
    }

    /// Query how many words are due.
    pub fn get_due_count(&mut self) -> Result<usize> {
        Ok(self.conn.query_row(
            "
            SELECT COUNT(*)
            FROM learn
            WHERE next < :now",
            named_params! { ":now": get_now() },
            |row| row.get(0),
        )?)
    }

    /// Retrieve a new word from the given list.  None indicates there is nothing left to learn on
    /// this list.
    pub fn get_new(&mut self, list: usize) -> Result<Option<Work>> {
        Ok(self
            .conn
            .query_row(
                "
            SELECT word, steno
            FROM lesson
            WHERE
                lesson.listid = :list AND
                lesson.word NOT IN (SELECT word FROM learn)
            ORDER BY seq
            LIMIT 1",
                named_params! {
                    ":list": list,
                },
                |row| {
                    let steno: String = row.get(1)?;
                    Ok(Work {
                        text: row.get(0)?,
                        strokes: StenoPhrase::parse(&steno).unwrap(),
                        goods: 0,
                        interval: 3.0,
                        next: 0.0,
                    })
                },
            )
            .optional()?)
    }

    /// Update the given work in the database.  `corrections` is the number of corrections the user
    /// had to make to write this.  For now, we consider 0 a success and will increase the good
    /// count and interval.
    pub fn update(&mut self, work: &Work, corrections: usize) -> Result<()> {
        let goods = if corrections == 0 {
            work.goods + 1
        } else {
            work.goods
        };
        let interval = if corrections == 0 {
            // Generate a random factor between 2.0 and 2.5.  This will distribute the resulting
            // times a bit randomly, keeping groups of words from being asked in the same order
            // each time.
            let bias = rand::random::<f64>() * 0.5;
            work.interval * (2.0 + bias)
        } else {
            5.0
        };
        let next = get_now() + interval;
        let steno = format!("{}", work.strokes);

        let tx = self.conn.transaction()?;
        tx.execute(
            "
            INSERT OR REPLACE INTO learn
            (word, steno, goods, interval, next)
            VALUES (:word, :steno, :goods, :interval, :next)",
            named_params! {
                ":steno": &steno,
                ":goods": goods,
                ":interval": interval,
                ":next": next,
                ":word": &work.text,
            },
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Retrieve a histogram of the number of words in range of dates.
    pub fn get_histogram(&mut self) -> Result<Vec<Bucket>> {
        let mut result: Vec<_> = BUCKETS
            .iter()
            .map(|b| Bucket {
                name: b.name,
                count: 0,
            })
            .collect();

        let mut stmt = self.conn.prepare("SELECT interval FROM learn")?;
        for interval in stmt.query_map([], |row| row.get::<usize, f64>(0))? {
            let interval = interval?;

            for (dest, src) in result.iter_mut().zip(BUCKETS) {
                if interval < src.limit {
                    dest.count += 1;
                    break;
                }
            }
        }

        Ok(result)
    }
}

/// Steno can be made as "Work" which is a linear sequence of strokes, and pieces of text that go
/// with each stroke.  For multiple stroke words, only the last stroke will include the text.  This
/// is similar to real behavior, but without the false words showing up first and then being
/// deleted.
#[derive(Clone, Debug)]
pub struct Work {
    pub text: String,
    pub strokes: StenoPhrase,
    pub goods: usize,
    pub interval: f64,
    pub next: f64,
    // pub items: Vec<WorkItem>,
}

// #[derive(Debug)]
// pub struct WorkItem {
//     pub text: String,
//     pub stroke: Stroke,
// }

// To simplify things a bit, we represent time as a floating point number of seconds since the Unix
// Epoch.  Get that time as a floating point value.  f64 up until 2037 gives 11 bits of precision
// left for sub-seconds.  We really only need a few bits of precision beyond seconds (even seconds
// would probably be fine).
fn get_now() -> f64 {
    let dur = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();

    dur.as_secs() as f64 + (dur.subsec_millis() as f64 / 1000.0)
}

struct InfoResult {
    id: i64,
    num: usize,
    total: usize,
    name: String,
}

/// Buckets describing a histogram result.
#[derive(Clone, Debug)]
pub struct Bucket {
    pub name: &'static str,
    pub count: u64,
}

/// The buckets
struct SrcBucket {
    name: &'static str,
    limit: f64,
}

static BUCKETS: &'static [SrcBucket] = &[
    SrcBucket {
        name: "fresh",
        limit: (10 * MIN) as f64,
    },
    SrcBucket {
        name: "10min",
        limit: HOUR as f64,
    },
    SrcBucket {
        name: "hour",
        limit: (6 * HOUR) as f64,
    },
    SrcBucket {
        name: "6hour",
        limit: DAY as f64,
    },
    SrcBucket {
        name: "day",
        limit: WEEK as f64,
    },
    SrcBucket {
        name: "week",
        limit: MONTH as f64,
    },
    SrcBucket {
        name: "month",
        limit: YEAR as f64,
    },
    SrcBucket {
        name: "solid",
        limit: f64::MAX,
    },
];

// Some useful time constants, all based on seconds.
const MIN: u64 = 60;
const HOUR: u64 = 60 * MIN;
const DAY: u64 = 24 * HOUR;
const WEEK: u64 = 7 * DAY;
const MONTH: u64 = 4 * WEEK;
const YEAR: u64 = 52 * WEEK;
