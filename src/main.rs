//! Steno learning application.

use anyhow::Result;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use crate::db::{Db, Work};
use crate::input::StrokeReader;
use crate::lessons::Lesson;
use std::io::Write;
use structopt::StructOpt;

mod db;
mod input;
mod lessons;
mod stroke;

#[derive(Debug, StructOpt)]
enum Command {
    #[structopt(name = "learn")]
    /// Learn and reinforce vocabulary.
    Learn(LearnCommand),

    #[structopt(name = "import")]
    /// Import wordlists to be learned.
    Import(ImportCommand),

    #[structopt(name = "init")]
    /// Initialize a new learning database
    Init(InitCommand),

    #[structopt(name = "info")]
    /// Return information about lesson progress
    Info(InfoCommand),
}

#[derive(Debug, StructOpt)]
struct ImportCommand {
    #[structopt(long = "db")]
    /// The pathname of the learning database
    file: String,

    #[structopt(name = "FILE")]
    files: Vec<String>,
}

#[derive(Debug, StructOpt)]
struct InitCommand {
    #[structopt(long = "db")]
    /// The pathname of the learning database
    file: String,
}

#[derive(Debug, StructOpt)]
struct InfoCommand {
    #[structopt(long = "db")]
    /// The pathname of the learning database
    file: String,

    #[structopt(long = "seen")]
    /// Only show seen entries.
    seen: bool,

    #[structopt(long = "unseen")]
    /// Only show unseen entries
    unseen: bool,
}

#[derive(Debug, StructOpt)]
struct LearnCommand {
    #[structopt(long = "db")]
    /// The pathname of the learning database.
    file: String,

    #[structopt(long = "new")]
    /// A lesson to pull new words from
    new: Option<usize>,
}

#[derive(Debug, StructOpt)]
#[structopt(name = "sdrill", about = "Steno drilling util")]
struct Opt {
    #[structopt(subcommand)]
    command: Command,
}

/// RawMode captures raw mode in a RAII so that error exit will still clear raw mode.
struct RawMode;

impl RawMode {
    fn new() -> Result<RawMode> {
        enable_raw_mode()?;
        Ok(RawMode)
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        // println!("\r\nDisabling raw mode\r");
        disable_raw_mode().unwrap();
    }
}

fn main() -> Result<()> {
    let opt = Opt::from_args();
    // println!("command: {:?}", opt);
    // let mut stdout = io::stdout();

    match opt.command {
        Command::Learn(args) => {
            let mut db = Db::open(&args.file)?;
            let _raw = RawMode::new()?;

            println!("Be sure Plover is configured to raw steno (no dict) and space after\r");
            println!("Press <Esc> to exit\r\n");

            learn(&mut db, args.new)?;
        }
        Command::Import(args) => {
            let mut db = Db::open(&args.file)?;

            for name in args.files {
                println!("import: {}", name);
                let lesson = Lesson::load(name)?;
                // println!("lesson: {:#?}", lesson);
                db.load(&lesson)?;
            }
        }

        Command::Init(args) => {
            println!("Initializing database at: {:?}", args.file);
            Db::init(&args.file)?;
        }

        Command::Info(args) => {
            let mut db = Db::open(&args.file)?;
            db.info(args.seen, args.unseen)?;
            println!();
            for bucket in db.get_histogram()? {
                if bucket.count > 0 {
                    println!("{:6}: {}", bucket.name, bucket.count);
                }
            }
        }
    }

    Ok(())
}

// Learn.
fn learn(db: &mut Db, new: Option<usize>) -> Result<()> {
    // let diag = Diagrammer::new();
    let mut reader = StrokeReader::new();

    loop {
        let words = db.get_drills(5)?;

        let head: Work;
        let rest: &[Work];
        if words.is_empty() {
            if let Some(list) = new {
                if let Some(work) = db.get_new(list)? {
                    println!("\nLearn new word: {}: {}\r", work.text, work.strokes);
                    head = work;
                    rest = &[];
                } else {
                    println!("No more words left in list.\r");
                    break;
                }
            } else {
                println!("No more words left to learn.\r");
                break;
            }
        } else {
            let (h, r) = words.split_at(1);
            head = h[0].clone();
            rest = r;
        };

        println!("{} words due\r", db.get_due_count()?);
        match learn_one(&mut reader, &head, rest)? {
            None => break,
            Some(count) => {
                db.update(&head, count)?;
            }
        }
    }

    Ok(())
}

// Quiz on a single word (with some context).  The value returned is the number of corrections
// needed.  A "None" return means the user wishes to exit.
fn learn_one(reader: &mut StrokeReader, work: &Work, rest: &[Work]) -> Result<Option<usize>> {
    let mut stdout = std::io::stdout();
    print!("Write: {}", work.text);

    let mut corrected = 0;
    let expect = work.strokes.linear();
    let mut sofar = vec![];

    if !rest.is_empty() {
        print!(" |");
    }
    for r in rest {
        print!(" {}", r.text);
    }
    println!("\r\n");
    loop {
        stdout.flush()?;

        let stroke = if let Some(stroke) = reader.read_stroke()? {
            stroke
        } else {
            println!("\r\n\nEarly exit\r");
            return Ok(None);
        };
        print!("--> ");

        if stroke.is_star() {
            let _ = sofar.pop();
            corrected += 1;
        } else {
            sofar.push(stroke);
        }

        let mut failed = false;

        for (pos, &st) in sofar.iter().enumerate() {
            if pos > 0 {
                print!("/");
            }
            if pos >= expect.len() || expect[pos] != st {
                failed = true;
                print!("%");
            }
            print!("{}", st);
        }
        println!("\r");

        if failed {
            println!("  Should be: {}\r", work.strokes);
        }

        if sofar.len() == expect.len() && !failed {
            println!("Correct! {}\r", corrected);
            return Ok(Some(corrected));
        }
    }
}
