// SPDX-License-Identifier: GPL-3.0
//! Learning database operations.

use crate::stroke::StenoPhrase;
use crate::Lesson;
use crate::ui::NewList;
use anyhow::{bail, Result};
use log::info;
use rusqlite::{named_params, Connection};
use std::collections::HashMap;
use std::path::Path;
use std::time::SystemTime;

/// The schema version that matches this code.  May be usable in the future for automatic upgrades.
static SCHEMA_VERSION: &str = "2022-03-12a";

static SCHEMA: &[&str] = &[
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
        UNIQUE (listid, seq));",
    // The history.  If 'stop' is null, then we didn't exit successfully.
    "CREATE TABLE history (
        entry TEXT NOT NULL,
        start DATETIME NOT NULL,
        stop DATETIME);",
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
                "  {:2}. {:5}/{:<5} ({:5}): {}{}",
                row.id,
                row.num,
                row.total,
                row.total - row.num,
                if row.num == row.total { '✓' } else { ' ' },
                row.name
            );
        }

        Ok(())
    }

    /// Query some words that need to be learned, returning up to count of them.
    pub fn get_learns(&mut self, count: usize) -> Result<Vec<Work>> {
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

    /// Retrieve a new word from the given lists.  When there are multiple lists, we try to work
    /// through the lists in a somewhat balanced manner.  There are two ways this could be done. 1.
    /// Based on how far each list has progressed, and select from the one the furthest behind, 2.
    /// Use the progress of each list as a percentage, and select randomly based on that.
    /// 1 has the advantage of picking a deterministic entry, but doesn't balance nicely across the
    /// lists.  Ideally, this should return from the entries, such that all of the lists will reach
    /// the end about the same time.  Returns None if all of the lists are empty.
    pub fn get_new(&mut self, list: &[NewList]) -> Result<Option<Work>> {
        // Wrap all of this up in a transaction that will be rolled back when we return.  This will
        // clean up the temp tables, which otherwise would survive through the database connection.
        let tx = self.conn.transaction()?;

        // The finder table is our lists to search for.
        tx.execute("CREATE TEMP TABLE finder (listid INTEGER REFERENCES list(id) NOT NULL)", [])?;
        for id in list {
            tx.execute("INSERT INTO finder VALUES (:id)",
                named_params!{ ":id": id.list })?;
        }

        // The minmax table caches the min and max values.  This is probably not needed because we
        // are only querying grouped results, but it does work.
        tx.execute("CREATE TEMP TABLE minmax AS
            SELECT listid, MIN(seq) AS seqmin, MAX(seq) AS seqmax
            FROM lesson
            WHERE lesson.word NOT IN (SELECT word FROM learn)
            GROUP BY listid", [])?;

        let mut stmt = tx.prepare(
            "SELECT word, steno,
                seqmax - seq + 1,
                lesson.listid
            FROM lesson, minmax
            WHERE lesson.listid IN finder AND
                lesson.listid = minmax.listid AND
                lesson.word NOT IN (SELECT word FROM learn)
            GROUP BY lesson.listid
            ORDER BY seq")?;
        let works: Vec<_> = stmt.query_map([], |row| {
            let steno: String = row.get(1)?;
            Ok(Minmax {
                word: row.get(0)?,
                steno: StenoPhrase::parse(&steno).unwrap(),
                progress: row.get(2)?,
                listid: row.get(3)?,
            })})?.collect();
        let works: rusqlite::Result<Vec<Minmax>> = works.into_iter().collect();
        let mut works = works?;

        let mut prog = 0.0f64;
        let pos = rand::random::<f64>();

        // Adjust all of the returned factors by the UI mult factors.
        let factors: HashMap<usize, f64> = list.iter().map(|n| (n.list, n.factor)).collect();

        for work in &mut works {
            work.progress += factors[&work.listid];
        }

        // Select among the words, randomly based on amount of progress through the lists.
        let total: f64 = works.iter().map(|w| w.progress).sum();

        info!("new word: prog={}, pos={}, total={}", prog, pos, total);
        for w in &works {
            info!("  work: {:?}", w);
        }

        for w in works {
            prog += w.progress;
            info!("check: prog={}, w={:?}", prog, w);
            if pos * total <= prog {
                return Ok(Some(Work {
                    text: w.word,
                    strokes: w.steno,
                    goods: 0,
                    interval: 3.0,
                    next: 0.0,
                }));
            }
        }
        Ok(None)
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
            // Generate a random factor between 2.5 and 3.0.  This will distribute the resulting
            // times a bit randomly, keeping groups of words from being asked in the same order
            // each time.
            let bias = rand::random::<f64>() * 0.5;
            work.interval * (2.5 + bias)
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

    /// Add a timestamp to the database.  Returns an ID needed to record the end stamp.
    pub fn start_timestamp(&mut self, key: &str) -> Result<i64> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO history (entry, start) VALUES (:entry, datetime())",
            named_params! { ":entry": key })?;
        let id = tx.last_insert_rowid();
        tx.commit()?;
        Ok(id)
    }

    pub fn stop_timestamp(&mut self, id: i64) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "UPDATE history SET stop = datetime()
            WHERE rowid = :id",
            named_params! { ":id": id })?;
        tx.commit()?;
        Ok(())
    }

    pub fn get_minutes_practiced(&mut self) -> Result<f64> {
        Ok(self.conn.query_row("
            SELECT SUM(24 * 60 * (julianday(stop) - julianday(start)))
            FROM history
            WHERE stop IS NOT NULL", [],
            |row| row.get(0))?)
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

/// Query results for getting work to do.
#[derive(Debug)]
struct Minmax {
    word: String,
    steno: StenoPhrase,
    progress: f64,
    listid: usize,
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
pub fn get_now() -> f64 {
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

static BUCKETS: &[SrcBucket] = &[
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
