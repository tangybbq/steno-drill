// SPDX-License-Identifier: GPL-3.0
//! Steno learning application.

use crate::db::Db;
use crate::lessons::Lesson;
use crate::ui::Ui;
use anyhow::Result;
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

    #[structopt(long = "tui")]
    /// Enable the TUI interface (deprecated)
    #[allow(dead_code)] // Deprecated: to be removed later
    tui: bool,
}

#[derive(Debug, StructOpt)]
#[structopt(name = "sdrill", about = "Steno drilling util")]
struct Opt {
    #[structopt(subcommand)]
    command: Command,
}

fn main() -> Result<()> {
    let opt = Opt::from_args();
    // println!("command: {:?}", opt);
    // let mut stdout = io::stdout();

    match opt.command {
        Command::Learn(args) => {
            let db = Db::open(&args.file)?;
            let mut ui = Ui::new(db, args.new)?;
            ui.run()?;
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
        }
    }

    Ok(())
}
