// SPDX-License-Identifier: GPL-3.0
//! UI for drill mode.

// For now, disable this. TODO: Remove this.
#![allow(unused_variables)]
#![allow(dead_code)]

use crate::db::{get_now, Db};
use crate::stroke::Stroke;
use super::{App, UiBackend};
use anyhow::Result;
use tui::{
    layout::{Constraint, Direction, Layout},
    terminal::Frame,
    widgets::{Block, Borders, List, ListItem},
};

#[derive(Default)]
pub struct DrillApp {
    list: usize,

    start_time: f64,
    learn_time: Option<usize>,
}

impl DrillApp {
    pub fn new(list: usize, repeat: Option<usize>, db: &mut Db) -> Result<DrillApp> {
        // Retrieve the words to drill.
        let mut drill = vec![];

        for _ in 0 .. repeat.unwrap_or(1) {
            let mut tmp = db.get_drill(list, 1, 10)?;
            drill.append(&mut tmp);
        }
        println!("drill: {:?}", drill.len());

        let start_time = get_now();
        Ok(DrillApp {
            start_time,
            list,
            ..DrillApp::default()
        })
    }
}

impl App for DrillApp {
    fn set_learntime(&mut self, learn_time: Option<usize>) {
        self.learn_time = learn_time;
    }

    fn goodbye_ref(&self) -> Option<&str> {
        Some("Goodbye")
    }

    fn update_status(&mut self, _db: &mut Db) -> Result<()> {
        Ok(())
    }

    fn update(&mut self, _db: &mut Db) -> Result<bool> {
        Ok(false)
    }

    fn add_stroke(&mut self, stroke: Stroke, db: &mut Db) -> Result<bool> {
        unimplemented!()
    }

    fn render(&mut self, f: &mut Frame<UiBackend>) {
        let top = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(45), Constraint::Length(25)].as_ref())
            .split(f.size());
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

        let lstatus = List::new([ListItem::new("Left Status")].as_ref())
            .block(Block::default().title("Status").borders(Borders::ALL));
        f.render_widget(lstatus, status[0]);

        let rstatus = List::new([ListItem::new("Right Status")].as_ref())
            .block(Block::default().title("Totals").borders(Borders::ALL));
        f.render_widget(rstatus, status[1]);

        // The exercise section gives the text to be typed.  We show this a list of 1 item so that
        // it doesn't try to wrap the text, even if the field grows.
        let items = [
            ListItem::new("this is what you should be writing")
        ];
        let exercise = List::new(items.as_ref())
            .block(Block::default().title("Exercise").borders(Borders::ALL));
        f.render_widget(exercise, left[1]);
    }
}
