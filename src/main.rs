// SPDX-License-Identifier: GPL-3.
//! Steno learning application.

use chrono::Local;
use crate::db::Db;
use crate::lessons::Lesson;
use crate::ui::{LearnApp, NewList, Ui};
use anyhow::Result;
use log::info;
use std::io::Write;
use std::fs::File;
use structopt::StructOpt;

mod db;
mod input;
mod lessons;
mod stroke;
mod ui;

#[derive(Debug, StructOpt)]
enum Command {
    #[structopt(name = "learn")]
    /// Learn and reinforce vocabulary.
    Learn(LearnCommand),

    #[structopt(name = "drill")]
    /// Drill a single list.
    Drill(DrillCommand),

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
    new: Vec<NewList>,

    #[structopt(long = "time")]
    /// Learn for the given number of minutes and exit.
    learn_time: Option<usize>,

    #[structopt(long = "tape")]
    /// Append strokes in tape format to given file
    tape_file: Option<String>,

    #[structopt(long = "tui")]
    /// Enable the TUI interface (deprecated)
    #[allow(dead_code)] // Deprecated: to be removed later
    tui: bool,
}

#[derive(Debug, StructOpt)]
struct DrillCommand {
    #[structopt(long = "db")]
    /// The pathname of the learning database.
    file: String,

    #[structopt(long = "list")]
    /// The lesson to drill.
    list: usize,

    #[structopt(long = "repeat")]
    /// The number of repetitions to drill.
    repeat: Option<usize>,

    #[structopt(long = "tape")]
    /// Append strokes in tape format to given file
    tape_file: Option<String>,
}

#[derive(Debug, StructOpt)]
#[structopt(name = "sdrill", about = "Steno drilling util")]
struct Opt {
    #[structopt(subcommand)]
    command: Command,
}

fn main() -> Result<()> {
    // TODO: Wrap the logger with one that, when in UI mode, logs to a section rather than spewing.
    // For now, expect that the program will be run with 2>logfile.
    env_logger::init();

    let opt = Opt::from_args();
    // println!("command: {:?}", opt);
    // let mut stdout = io::stdout();

    match opt.command {
        Command::Learn(args) => {
            info!("Starting learn mode");
            let tapefile = args.tape_file.as_ref().map(|n| open_tape_file(n)).transpose()?;
            let tapefile = tapefile.map(|f| Box::new(f) as Box<dyn Write>);
            let db = Db::open(&args.file)?;
            let app = LearnApp::new_learn(args.new);
            let mut ui = Ui::new(db, Box::new(app), tapefile)?;
            ui.run(args.learn_time)?;
        }

        Command::Drill(args) => {
            info!("Starting drill mode");
            let tapefile = args.tape_file.as_ref().map(|n| open_tape_file(n)).transpose()?;
            let tapefile = tapefile.map(|f| Box::new(f) as Box<dyn Write>);
            let db = Db::open(&args.file)?;
            let _ = args.repeat;
            let app = LearnApp::new_drill(args.list);
            let mut ui = Ui::new(db, Box::new(app), tapefile)?;
            ui.run(None)?;
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
            let due = db.get_due_count()?;
            println!();
            println!("{} words due", due);
            let hist = db.get_histogram()?;
            for bucket in &hist {
                if bucket.count > 0 {
                    println!("{:6}: {}", bucket.name, bucket.count);
                }
            }
            println!("------: ----");
            println!("{:6}: {}", "", hist.iter().map(|b| b.count).sum::<u64>());
            println!("{:.1} minutes practiced",
                db.get_minutes_practiced()?);
        }
    }

    Ok(())
}

fn open_tape_file(name: &str) -> Result<File> {
    let mut fd = File::options().write(true).append(true).create(true).open(name)?;
    let now = Local::now();
    writeln!(fd, "{}", now)?;
    Ok(fd)
}
