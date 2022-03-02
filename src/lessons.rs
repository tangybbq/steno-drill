//! Processing of lessons.

use crate::stroke::StenoPhrase;
use anyhow::{anyhow, bail, Result};
use std::{
    fs::File,
    io::{self, BufRead, BufReader},
    path::Path,
};

#[derive(Debug)]
pub struct Entry {
    pub word: String,
    pub steno: StenoPhrase,
}

#[derive(Debug)]
pub struct Lesson {
    pub description: String,
    pub entries: Vec<Entry>,
}

impl Lesson {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Lesson> {
        let mut inp = BufReader::new(File::open(path)?).lines();

        let description = oneline(&mut inp)?;
        let blank = oneline(&mut inp)?;
        if !blank.is_empty() {
            bail!("Expecting lesson file to have a blank second line");
        }

        println!("Description: {}", description);

        let mut entries = vec![];

        for line in inp {
            let line = line?;
            if let Some(entry) = Entry::parse(&line)? {
                entries.push(entry);
            } else {
                println!("  {}", line);
            }
        }

        Ok(Lesson {
            description,
            entries,
        })
    }
}

impl Entry {
    // Parse this line as an entry.  Can return Ok(None) if this line doesn't start with a '\''
    // character, or have a colon.  May return an error if there was a problem decoding the line.
    // Entries are expected to have the format:
    // 'text': STENO
    // where text is an _arbitrary_ string (which may include single quotes".
    fn parse(text: &str) -> Result<Option<Entry>> {
        let fields: Vec<_> = text.splitn(2, ": ").collect();
        if fields.len() != 2 {
            return Ok(None);
        }

        let word = fields[0];
        if word.len() < 2 || !word.starts_with('\'') || !word.ends_with('\'') {
            // If the extra data has a colon in it, it will trigger this.
            println!("warning: {}: {}", word, fields[1]);
            return Ok(None)
            // bail!("Looks like entry, but word is not surrounded by ''");
        }
        let word = &word[1..word.len() - 1];
        let word = word.to_string();

        let steno = StenoPhrase::parse(fields[1])?;

        Ok(Some(Entry { word, steno }))
    }
}

// Read a single line from the reader, returning an error if we've reached the end.
fn oneline<B>(rd: &mut io::Lines<B>) -> Result<String>
where
    B: BufRead,
{
    Ok(rd
        .next()
        .ok_or_else(|| anyhow!("Unexpected EOF on lesson file"))??)
}
