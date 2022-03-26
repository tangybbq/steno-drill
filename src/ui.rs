// SPDX-License-Identifier: GPL-3.0
//! The textual ui.

use crate::db::{get_now, Db};
use crate::input::{StrokeReader, Value};
use crate::stroke::{Stroke};
use anyhow::Result;
use learn::LearnApp;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{
    io::{self, Write},
    time::Duration,
};
use tui::{
    backend::{CrosstermBackend},
    layout::Rect,
    terminal::Frame,
    Terminal,
};

mod learn;

type UiBackend = CrosstermBackend<std::io::Stdout>;

pub struct Ui {
    terminal: Terminal<UiBackend>,
    app: Box<dyn App>,
    reader: StrokeReader,
    db: Db,

    // A possible place to record strokes.
    tapefile: Option<Box<dyn Write>>,
}

/// The application is controlled via this trait.
trait App {
    fn update_status(&mut self, db: &mut Db) -> Result<()>;
    fn update(&mut self, db: &mut Db) -> Result<bool>;
    fn add_stroke(&mut self, stroke: Stroke, db: &mut Db) -> Result<bool>;

    fn set_learntime(&mut self, learn_time: Option<usize>);
    fn goodbye_ref(&self) -> Option<&str>;

    fn render(&mut self, f: &mut Frame<UiBackend>);
}

impl Ui {
    pub fn new(db: Db, new: Vec<NewList>, tapefile: Option<Box<dyn Write>>) -> Result<Ui> {
        let mut stdout = io::stdout();
        enable_raw_mode()?;
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        let now = get_now();
        let app = LearnApp::new(now, new);
        let reader = StrokeReader::new();

        Ok(Ui {
            terminal,
            app: Box::new(app),
            reader,
            db,
            tapefile: tapefile,
        })
    }

    pub fn run(&mut self, learn_time: Option<usize>) -> Result<()> {
        self.app.set_learntime(learn_time);
        if self.app.update(&mut self.db)? {
            return Ok(());
        }
        let stamp_id = self.db.start_timestamp("learn")?;
        loop {
            self.app.update_status(&mut self.db)?;

            self.terminal.draw(|f| self.app.render(f))?;

            match self.reader.read_stroke(Duration::from_secs(1))? {
                Value::Stroke(stroke) => {
                    if let Some(tf) = &mut self.tapefile {
                        writeln!(tf, "{}", stroke.to_tape())?;
                    }

                    if self.app.add_stroke(stroke, &mut self.db)? {
                        break;
                    }
                }
                Value::Resize(width, height) => self.terminal.resize(Rect {
                    x: 1,
                    y: 1,
                    width,
                    height,
                })?,
                Value::Exit => break,
                Value::Timeout => (),
            }
        }
        self.db.stop_timestamp(stamp_id)?;
        Ok(())
    }
}

impl Drop for Ui {
    fn drop(&mut self) {
        disable_raw_mode().unwrap();
        execute!(self.terminal.backend_mut(), LeaveAlternateScreen).unwrap();
        self.terminal.show_cursor().unwrap();

        if let Some(message) = self.app.goodbye_ref() {
            println!("{}", message);
        }
    }
}

/// New words have a list ID associated with a multiplication factor to bias toward certain lists.
#[derive(Debug)]
pub struct NewList {
    pub list: usize,
    pub factor: f64,
}

// Implement so it can be used from the UI.  We accept either a single integer, or an int:float
// pair.  The factor will be '0' if not specified.
impl std::str::FromStr for NewList {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<NewList> {
        let fields: Vec<_> = s.splitn(2, ':').collect();
        let list: usize = fields[0].parse()?;
        let factor: f64 = fields.get(1).map(|f| f.parse()).unwrap_or(Ok(0.0))?;
        Ok(NewList { list, factor })
    }
}
