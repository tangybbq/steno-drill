//! The textual ui.

use crate::db::{Db, Work};
use crate::input::{StrokeReader, Value};
use crate::stroke::{Stroke, StenoWord};
use anyhow::Result;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{collections::VecDeque, io};
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
}

impl Ui {
    pub fn new(db: Db) -> Result<Ui> {
        let mut stdout = io::stdout();
        enable_raw_mode()?;
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        let app = App::new();
        let reader = StrokeReader::new();

        Ok(Ui {
            terminal,
            app,
            reader,
            db,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        if self.update()? {
            return Ok(());
        }
        loop {
            self.update_status()?;

            self.terminal.draw(|f| self.app.render(f))?;

            match self.reader.read_stroke()? {
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
            }
        }
        Ok(())
    }

    // Update the status, based on the information from the database.
    fn update_status(&mut self) -> Result<()> {
        let due = self.db.get_due_count()?;

        self.app.status.clear();
        self.app.status.push(ListItem::new(format!("words due: {}", due)));

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

        if words.is_empty() {
            // New list, for now, just exit.
            return Ok(true);
        }

        self.app.text.clear();
        self.app.sofar.clear();
        self.app.expected.clear();

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

        self.app.corrected = 0;
        self.app.help = None;

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
            items.push(ListItem::new("-"));
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
    }
}
