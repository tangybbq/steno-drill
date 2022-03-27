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
    terminal::Frame,
};

#[derive(Default)]
pub struct DrillApp {
    list: usize,
    repeat: usize,

    start_time: f64,
    learn_time: Option<usize>,
}

impl DrillApp {
    pub fn new(list: usize, repeat: Option<usize>) -> DrillApp {
        let start_time = get_now();
        DrillApp {
            start_time,
            list,
            repeat: repeat.unwrap_or(1),
            ..DrillApp::default()
        }
    }
}

impl App for DrillApp {
    fn set_learntime(&mut self, learn_time: Option<usize>) {
        self.learn_time = learn_time;
    }

    fn goodbye_ref(&self) -> Option<&str> {
        Some("Goodbye")
    }

    fn update_status(&mut self, db: &mut Db) -> Result<()> {
        unimplemented!()
    }

    fn update(&mut self, db: &mut Db) -> Result<bool> {
        unimplemented!()
    }

    fn add_stroke(&mut self, stroke: Stroke, db: &mut Db) -> Result<bool> {
        unimplemented!()
    }

    fn render(&mut self, f: &mut Frame<UiBackend>) {
        unimplemented!()
    }
}
