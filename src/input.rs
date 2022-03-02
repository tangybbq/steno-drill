//! Management of keyboard input.
//!
//! Rather than trying to implement steno protocols, we make use of Plover.  By disabling the
//! dictionary, and configuring plover to output a space after each stroke, we get the advantage of
//! seeing the full strokes.
//!
//! However, Plover still tracks how many characters it has typed, and pressing '*' will remove
//! that many characters.  To accomodate this, we will keep track of how many characters are
//! received, including the space, and when backspace is received, subtract from that until we
//! cross a boundary.

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use std::collections::VecDeque;

use crate::stroke::Stroke;

pub struct StrokeReader {
    sizes: VecDeque<usize>,
}

impl StrokeReader {
    pub fn new() -> StrokeReader {
        StrokeReader {
            sizes: VecDeque::new(),
        }
    }

    /// Attempt to read a stroke from the input.  Returns Ok(None) when Escape is pressed, to
    /// indicate the user wishes to exit.
    pub fn read_stroke(&mut self) -> Result<Option<Stroke>> {
        let mut buffer = String::new();

        loop {
            match event::read()? {
                Event::Key(KeyEvent {
                    code: KeyCode::Esc, ..
                }) => return Ok(None),
                Event::Key(KeyEvent {
                    code: KeyCode::Char(' '),
                    ..
                }) => break,
                Event::Key(KeyEvent {
                    code: KeyCode::Char(ch),
                    ..
                }) => buffer.push(ch),
                Event::Key(KeyEvent {
                    code: KeyCode::Backspace,
                    ..
                }) => {
                    if buffer.is_empty() {
                        // Pop a stroke.
                        let count = if let Some(count) = self.sizes.pop_back() {
                            count
                        } else {
                            println!("Warning, backspace before input\r");
                            continue;
                        };
                        match count {
                            0 => panic!("Should never push 0"),
                            1 => {
                                // Word boundary, return the deletion up, and leave the stroke
                                // popped.
                                // println!("Return *\r");
                                return Ok(Some(Stroke::from_text("*")?));
                            }
                            n => {
                                // Not word boundary, just reduce the count.
                                self.sizes.push_back(n - 1);
                            }
                        }
                    } else {
                        println!("TODO: Backspace in a word");
                        return Ok(None);
                    }
                }
                _ => (),
            }
        }

        self.sizes.push_back(buffer.len() + 1);
        while self.sizes.len() > 100 {
            _ = self.sizes.pop_front();
        }

        Ok(Some(Stroke::from_text(&buffer)?))
    }
}
