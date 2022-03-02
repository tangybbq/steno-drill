//! Steno learning application.

use anyhow::Result;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use crate::db::Db;
use crate::input::StrokeReader;
use crate::{lessons::Lesson, stroke::Diagrammer};
use structopt::StructOpt;

mod db;
mod input;
mod lessons;
mod stroke;

#[derive(Debug, StructOpt)]
enum Command {
    #[structopt(name = "learn")]
    /// Learn and reinforce vocabulary.
    Learn,

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
        Command::Learn => {
            let _raw = RawMode::new()?;

            println!("Be sure Plover is configured to raw steno (no dict) and space after\r");
            println!("Press <Esc> to exit\r\n");
            // crossterm::execute!(
            //     stdout,
            //     enable_raw_mode(),
            // )?;

            learn()?;
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
        }
    }

    Ok(())
}

// Learn.
fn learn() -> Result<()> {
    let diag = Diagrammer::new();
    let mut reader = StrokeReader::new();

    while let Some(stroke) = reader.read_stroke()? {
        println!("read: |{}|  {}\r", stroke.to_tape(), stroke);
        for row in diag.to_diagram(stroke) {
            println!("  > {}\r", row);
        }
    }

    Ok(())
}
