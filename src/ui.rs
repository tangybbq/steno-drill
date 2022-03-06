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
    new: Option<usize>,

    last_time: f64,
    start_time: f64,

    // A goodbye message.
    goodbye: Option<String>,
}

// State of the application.
#[derive(Default)]
struct App {
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
}

impl Ui {
    pub fn new(db: Db, new: Option<usize>) -> Result<Ui> {
        let mut stdout = io::stdout();
        enable_raw_mode()?;
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        let app = App::new();
        let reader = StrokeReader::new();
        let now = get_now();

        Ok(Ui {
            terminal,
            app,
            reader,
            db,
            new,
            last_time: now,
            start_time: now,
            goodbye: None,
        })
    }

    pub fn run(&mut self, learn_time: Option<usize>) -> Result<()> {
        if self.update()? {
            return Ok(());
        }
        loop {
            self.update_status()?;

            self.terminal.draw(|f| self.app.render(f))?;

            match self.reader.read_stroke(Duration::from_secs(1))? {
                Value::Stroke(stroke) => {
                    self.app.tape.push_front(stroke);
                    if self.app.tape.len() > 1000 {
                        _ = self.app.tape.pop_back();
                    }

                    if self.add_stroke(stroke)? {
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

            if let Some(max_time) = learn_time {
                let now = get_now();
                if now - self.start_time > (max_time as f64 * 60.0) {
                    self.goodbye = Some("Lesson learn time reached".to_string());
                    break;
                }
            }
        }
        Ok(())
    }

    // Update the status, based on the information from the database.
    fn update_status(&mut self) -> Result<()> {
        let due = self.db.get_due_count()?;

        let now = get_now();
        self.app.elapsed = (now - self.start_time) as usize;

        self.app.status.clear();
        self.app.status.push(ListItem::new(
                format!("Elapsed {:02}:{:02}",
                    self.app.elapsed / 60,
                    self.app.elapsed % 60)));
        self.app.status.push(ListItem::new(format!("words due: {}", due)));
        self.app.status.push(ListItem::new(format!("new words: {}", self.app.new_words)));
        self.app.status.push(ListItem::new(format!("WPM: {:.1}", self.app.wpm)));
        // self.app.status.push(ListItem::new(format!("factor: {:.4}", self.app.factor)));

        self.app.rstatus.clear();
        let hist = self.db.get_histogram()?;
        for bucket in &hist {
            if bucket.count > 0 {
                self.app.rstatus.push(
                    ListItem::new(format!("{:6}: {}", bucket.name, bucket.count)));
            }
        }
        self.app.rstatus.push(
            ListItem::new(format!("total : {}", hist.iter().map(|b| b.count).sum::<u64>())));

        Ok(())
    }

    // Update the app with the current progress.  Returns true if we should exit.
    fn update(&mut self) -> Result<bool> {
        let words = self.db.get_drills(20)?;

        self.app.text.clear();
        self.app.sofar.clear();
        self.app.expected.clear();
        self.app.corrected = 0;
        self.app.help = None;

        if words.is_empty() {
            if let Some(list) = self.new {
                if let Some(work) = self.db.get_new(list)? {
                    self.app.expected.append(&mut work.strokes.linear());
                    self.app.text.push_str(&work.text);
                    self.app.help = Some(format!("New word: {}", work.strokes));
                    self.app.head = Some(work);
                    self.app.new_words += 1;
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
                    self.app.text.push(' ');
                }
                self.app.text.push_str(&word.text);
                if id == 0 {
                    self.app.expected.append(&mut word.strokes.linear());
                    self.app.text.push_str(" |");
                }
            }
            if let Some(head) = words.first() {
                self.app.head = Some(head.clone());
            } else {
                unreachable!();
            }
        }

        Ok(false)
    }

    // Add a single stroke that the user has written.  If it matches, will call 'update' to
    // move to the next thing to write.  Otherwise, status will remain, showing the user any
    // errors.  Will return Ok(true) if we have run out of things to do.
    fn add_stroke(&mut self, stroke: Stroke) -> Result<bool> {
        if stroke.is_star() {
            _ = self.app.sofar.pop();
            self.app.corrected += 1;
        } else {
            self.app.sofar.push(stroke);
        }

        if self.app.expected == self.app.sofar {
            // Update the WPM.
            let now = get_now();
            let new_wpm = 60.0 / (now - self.last_time);
            self.last_time = now;
            self.app.wpm = self.app.factor * self.app.wpm +
                (1.0 - self.app.factor) * new_wpm;

            // Adjust the factor, so it gradually increases from 0 to 0.95.
            self.app.factor = 1.0 - ((0.95 - self.app.factor) * 0.9 + 0.05);

            // Written correctly, record this, and update.
            self.db.update(self.app.head.as_ref().unwrap(), self.app.corrected)?;
            self.update()
        } else {
            // Check for any errors, and show a hint if that happens.
            let mut show = false;
            for (&a, &b) in self.app.expected.iter().zip(&self.app.sofar) {
                if a != b {
                    show = true;
                }

                if show {
                    let strokes = StenoWord(self.app.expected.clone());
                    self.app.help = Some(format!("Should be written as {}", strokes));
                }
            }
            Ok(false)
        }
    }
}

impl App {
    fn new() -> App {
        App::default()
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
                Constraint::Length(10),
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

        if let Some(message) = &self.goodbye {
            println!("{}", message);
        }
    }
}
