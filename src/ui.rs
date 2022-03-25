// SPDX-License-Identifier: GPL-3.0
//! The textual ui.

use crate::db::{get_now, Db, Work};
use crate::input::{StrokeReader, Value};
use crate::stroke::{Stroke, StenoWord};
use anyhow::Result;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{
    collections::VecDeque, io,
    io::Write,
    time::Duration,
};
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    terminal::Frame,
    text::{Span, Spans},
    widgets::{Block, Borders, List, ListItem},
    Terminal,
};

pub struct Ui {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    app: App,
    reader: StrokeReader,
    db: Db,

    // A possible place to record strokes.
    tapefile: Option<Box<dyn Write>>,
}

// State of the application.
#[derive(Default)]
struct App {
    // The lists to get new entries from.
    new: Vec<NewList>,

    // The tape represents everything stroked, as a tape from the steno machine would look.
    // New entries are pushed to the front.
    tape: VecDeque<Stroke>,

    // The current status
    status: Vec<ListItem<'static>>,
    rstatus: Vec<ListItem<'static>>,

    // The text represents what we are asking the user to write.
    text: String,

    // This shows strokes that have been written so far.
    sofar: Vec<Stroke>,

    // These are the strokes the user is expected to write.
    expected: Vec<Stroke>,

    help: Option<String>,

    // Did the user have to correct the currently written stroke?
    corrected: usize,

    // The database entry for the word being written, needed to update the database.
    head: Option<Work>,

    // Average WPM
    wpm: f64,

    // The factor used to decay the WPM.  This is the amount of the previous value used to compute
    // the updated WPM.  It starts at 0 and works its way up to 0.95.
    factor: f64,

    // New words that have been learned.
    new_words: usize,

    // Number of seconds since the drill was started.
    elapsed: usize,

    // A learn time, in minutes.
    learn_time: Option<usize>,

    // The time this invocation was started (needed to show the display).
    start_time: f64,
    last_time: f64,

    // A goodbye message.
    goodbye: Option<String>,
}

impl Ui {
    pub fn new(db: Db, new: Vec<NewList>, tapefile: Option<Box<dyn Write>>) -> Result<Ui> {
        let mut stdout = io::stdout();
        enable_raw_mode()?;
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        let now = get_now();
        let app = App::new(now, new);
        let reader = StrokeReader::new();

        Ok(Ui {
            terminal,
            app,
            reader,
            db,
            tapefile: tapefile,
        })
    }

    pub fn run(&mut self, learn_time: Option<usize>) -> Result<()> {
        self.app.learn_time = learn_time;
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

impl App {
    fn new(start_time: f64, new: Vec<NewList>) -> App {
        App {
            start_time,
            last_time: start_time,
            new,
            ..App::default()
        }
    }

    /// Update the status of the app, based on information from the database.
    fn update_status(&mut self, db: &mut Db) -> Result<()> {
        let due = db.get_due_count()?;

        let now = get_now();
        self.elapsed = (now - self.start_time) as usize;

        self.status.clear();
        if let Some(limit) = self.learn_time {
            self.status.push(ListItem::new(
                    format!("Elapsed {:02}:{:02} / {:02}:00",
                        self.elapsed / 60,
                        self.elapsed % 60,
                        limit)));
        } else {
            self.status.push(ListItem::new(
                    format!("Elapsed {:02}:{:02}",
                        self.elapsed / 60,
                        self.elapsed % 60)));
        }
        self.status.push(ListItem::new(format!("words due: {}", due)));
        self.status.push(ListItem::new(format!("new words: {}", self.new_words)));
        self.status.push(ListItem::new(format!("WPM: {:.1}", self.wpm)));
        // self.app.status.push(ListItem::new(format!("factor: {:.4}", self.app.factor)));

        self.rstatus.clear();
        let hist = db.get_histogram()?;
        for bucket in &hist {
            if bucket.count > 0 {
                self.rstatus.push(
                    ListItem::new(format!("{:6}: {}", bucket.name, bucket.count)));
            }
        }
        self.rstatus.push(
            ListItem::new(format!("total : {}", hist.iter().map(|b| b.count).sum::<u64>())));

        Ok(())
    }

    // Update the app with the current progress.  Returns true if we should exit.
    fn update(&mut self, db: &mut Db) -> Result<bool> {
        let words = db.get_drills(20)?;

        self.text.clear();
        self.sofar.clear();
        self.expected.clear();
        self.corrected = 0;
        self.help = None;

        let mut new_word = false;
        if words.is_empty() {
            if !self.new.is_empty() {
                if let Some(work) = db.get_new(&self.new)? {
                    self.expected.append(&mut work.strokes.linear());
                    self.text.push_str(&work.text);
                    self.head = Some(work);
                    self.new_words += 1;
                    new_word = true;
                } else {
                    self.goodbye = Some("No more words left in list.".to_string());
                    return Ok(true);
                }
            } else {
                self.goodbye = Some("No more words left to learn.".to_string());
                return Ok(true);
            }
        } else {
            for (id, word) in words.iter().enumerate() {
                if id > 0 {
                    self.text.push(' ');
                }
                self.text.push_str(&word.text);
                if id == 0 {
                    self.expected.append(&mut word.strokes.linear());
                    self.text.push_str(" |");
                }
            }
            if let Some(head) = words.first() {
                self.head = Some(head.clone());
            } else {
                unreachable!();
            }
        }

        if let Some(work) = &self.head {
            if work.interval < 90.0 {
                self.help = Some(format!("{}write: {}",
                        if new_word { "New word, " } else { "" },
                        work.strokes));
            }
        }

        Ok(false)
    }

    /// Add a single stroke that the user has written.  If it matches, will call 'update' to
    /// move to the next thing to write.  Otherwise, status will remain, showing the user any
    /// errors.  Will return Ok(true) if we have run out of things to do.
    fn add_stroke(&mut self, stroke: Stroke, db: &mut Db) -> Result<bool> {
        // The tape always records the strokes, as written.  Store in the tape before any kind of
        // processing.
        self.tape.push_front(stroke);
        if self.tape.len() > 1000 {
            _ = self.tape.pop_back();
        }

        if stroke.is_star() {
            _ = self.sofar.pop();
            self.corrected += 1;
        } else {
            self.sofar.push(stroke);
        }

        if self.expected == self.sofar {
            // Update the WPM.
            let now = get_now();
            let new_wpm = 60.0 / (now - self.last_time);
            self.last_time = now;
            self.wpm = self.factor * self.wpm +
                (1.0 - self.factor) * new_wpm;

            // Adjust the factor, so it gradually increases from 0 to 0.95.
            self.factor = 1.0 - ((0.95 - self.factor) * 0.9 + 0.05);

            // Written correctly, record this, and update.
            db.update(self.head.as_ref().unwrap(), self.corrected)?;
            if self.update(db)? {
                return Ok(true);
            }

            // Check if we have reached the expired time.
            if let Some(max_time) = self.learn_time {
                let now = get_now();
                if now - self.start_time > (max_time as f64 * 60.0) {
                    self.goodbye = Some("Lesson learn time reached.".to_string());
                    return Ok(true);
                }
            }
            Ok(false)
        } else {
            // Check for any errors, and show a hint if that happens.
            let mut show = false;
            for (&a, &b) in self.expected.iter().zip(&self.sofar) {
                if a != b {
                    show = true;
                }

                if show {
                    let strokes = StenoWord(self.expected.clone());
                    self.help = Some(format!("Should be written as {}", strokes));
                }
            }
            Ok(false)
        }
    }

    fn render<B: Backend>(&mut self, f: &mut Frame<B>) {
        let top = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(45), Constraint::Length(25)].as_ref())
            .split(f.size());
        // We kind of assume a particular layout.
        let left = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(12),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Max(70),
            ])
            .split(top[0]);

        let status = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
            .split(left[0]);

        let lstatus = List::new(self.status.as_ref())
            .block(Block::default().title("Status").borders(Borders::ALL));
        f.render_widget(lstatus, status[0]);

        let rstatus = List::new(self.rstatus.as_ref())
            .block(Block::default().title("Totals").borders(Borders::ALL));
        f.render_widget(rstatus, status[1]);

        // The Exercise section gives the text to be shown.  We show this as a list of 1 item so
        // that it doesn't try to wrap the text, even if the field grows.
        let items = [
            ListItem::new(self.text.as_ref())
        ];
        let exercise = List::new(items.as_ref())
            .block(Block::default().title("Exercise").borders(Borders::ALL));
        f.render_widget(exercise, left[1]);

        let mut spans = vec![];
        for (id, &stroke) in self.sofar.iter().enumerate() {
            if id > 0 {
                spans.push(Span::raw(" / "));
            }
            let textual = format!("{}", stroke);
            if id >= self.expected.len() || stroke != self.expected[id] {
                spans.push(Span::styled(textual, Style::default().add_modifier(Modifier::REVERSED)));
            } else {
                spans.push(Span::raw(textual));
            }
        }
        let strokes = List::new([ListItem::new(Spans(spans))].as_ref())
            .block(Block::default().title("Strokes").borders(Borders::ALL));
        f.render_widget(strokes, left[2]);

        let mut items = vec![];
        if let Some(text) = &self.help {
            items.push(ListItem::new(text.as_ref()));
        }
        let help = List::new(items.as_slice())
            .block(Block::default().title("Help").borders(Borders::ALL));
        f.render_widget(help, left[3]);

        // Render the tape.
        let mut items = vec![];
        let height = (top[1].height - 2) as usize;
        // Pull in enough of the tape to fill our space.
        for stroke in &self.tape {
            if items.len() >= height {
                break;
            }
            items.push(ListItem::new(stroke.to_tape()));
        }
        // Add blank lines to fill the available space.
        while items.len() < height {
            items.push(ListItem::new(""));
        }
        items.reverse();
        let tape = List::new(items).block(Block::default().title("Tape").borders(Borders::ALL));
        f.render_widget(tape, top[1]);
    }
}

impl Drop for Ui {
    fn drop(&mut self) {
        disable_raw_mode().unwrap();
        execute!(self.terminal.backend_mut(), LeaveAlternateScreen).unwrap();
        self.terminal.show_cursor().unwrap();

        if let Some(message) = &self.app.goodbye {
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
