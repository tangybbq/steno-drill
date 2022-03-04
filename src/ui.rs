//! The textual ui.

use crate::input::{StrokeReader, Value};
use crate::stroke::Stroke;
use anyhow::Result;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{collections::VecDeque, io};
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout, Rect},
    terminal::Frame,
    widgets::{Block, Borders, List, ListItem},
    Terminal,
};

pub struct Ui {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    app: App,
    reader: StrokeReader,
}

// State of the application.
struct App {
    // The tape represents everything stroked, as a tape from the steno machine would look.
    // New entries are pushed to the front.
    tape: VecDeque<Stroke>,
}

impl Ui {
    pub fn new() -> Result<Ui> {
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
        })
    }

    pub fn run(&mut self) -> Result<()> {
        loop {
            self.terminal.draw(|f| self.app.render(f))?;

            match self.reader.read_stroke()? {
                Value::Stroke(stroke) => {
                    self.app.tape.push_front(stroke);
                    if self.app.tape.len() > 1000 {
                        _ = self.app.tape.pop_back();
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
}

impl App {
    fn new() -> App {
        App {
            tape: VecDeque::new(),
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
                Constraint::Length(6),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Max(70),
            ])
            .split(top[0]);

        let status = Block::default().title("Status").borders(Borders::ALL);
        f.render_widget(status, left[0]);

        let drill = Block::default().title("Exercise").borders(Borders::ALL);
        f.render_widget(drill, left[1]);

        let strokes = Block::default().title("Strokes").borders(Borders::ALL);
        f.render_widget(strokes, left[2]);

        let help = Block::default().title("Help").borders(Borders::ALL);
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
