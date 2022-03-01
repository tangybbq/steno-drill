//! Steno stroke encoding.
//!
//! A (US) steno stroke consists of the following characters: #STKPWHRAO*EUFRPBLGTSDZ which can be
//! provided in nearly any combination (subject to the limits of the human hand to press them.  We
//! will represent them by binary.  The textual representation is mostly just the characters
//! present, when that bit is set.  However, if the right section is present, and the middle
//! section is not, there will be a single '-' before the right characters (otherwise, the stroke
//! might be ambiguous).
//!
//! Our parser is currently fairly strict and requires the hyphen to be present.
//!
//! The number bar can be textually represented by the '#' if needed to disambiguate.  If there are
//! any number row characters present, the '#' is not needed.

use anyhow::{
    bail,
    Result,
};
use std::{
    fmt,
};

// The stroke itself is just a 32 bit number.
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct Stroke(u32);

static NORMAL: &str = "STKPWHRAO*EUFRPBLGTSDZ";
static NUMS: &str   = "12K3W4R50*EU6R7B8G9SDZ";

// #ST KPWH RAO* EURF PBLG TSDZ

// Various masks.
// static LEFT: Stroke = Stroke(0x7f8000);
static MID: Stroke = Stroke(0x007c00);
static RIGHT: Stroke = Stroke(0x0003ff);
static NUM: Stroke = Stroke(0x400000);
static DIGITS: Stroke = Stroke(0x3562a8);
// static STAR: Stroke = Stroke(0x001000);

impl Stroke {
    pub fn from_text(text: &str) -> Result<Stroke> {
        let mut result = 0u32;
        let mut bit = NUM.0;
        let mut must_not_num = false;

        let mut norms = NORMAL.chars();
        let mut nums = NUMS.chars();

        for ch in text.chars() {
            if ch == '#' {
                result |= NUM.0;
                continue;
            }

            if ch == '-' {
                if bit < MID.0 {
                    bail!("Invalid placement of '-' in stroke");
                }

                while bit > MID.0 {
                    bit >>= 1;
                    if let Some(_) = norms.next() {
                    } else {
                        panic!("State error");
                    }
                    if let Some(_) = nums.next() {
                    } else {
                        panic!("State error");
                    }
                }

                continue;
            }

            loop {
                // Get the next normal a numeric character, and the next bit to go with that.
                bit >>= 1;
                let norm = if let Some(n) = norms.next() {
                    n
                } else {
                    bail!("Invalid character: {} in stroke", ch);
                };
                let num = if let Some(n) = nums.next() {
                    n
                } else {
                    panic!("Unexpected state");
                };

                if ch == norm {
                    result |= bit;
                    if ch != num {
                        must_not_num = true;
                    }
                    break;
                } else if ch == num {
                    result |= bit | NUM.0;
                    break;
                }

                // The character didn't match, go on to the next one.
            }
        }

        if (result & NUM.0) != 0 && must_not_num {
            bail!("Stroke has # and inappropriate character");
        }
        Ok(Stroke(result))
    }

    /// Determine if this stroke has any of the keys pressed in 'other'.
    pub fn has_any(self, other: Stroke) -> bool {
        (self.0 & other.0) != 0
    }
}

// Display is in canoncal order.
impl fmt::Display for Stroke {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // The '#' should be printed if the number is present, but none of the digits are present.
        if self.has_any(NUM) && !self.has_any(DIGITS) {
            write!(f, "#")?;
        }
        let need_hyphen = self.has_any(RIGHT) && !self.has_any(MID);
        let chars = if self.has_any(NUM) { NUMS } else { NORMAL };
        let mut bit = NUM.0 >> 1;
        for ch in chars.chars() {
            if ch == '*' && need_hyphen {
                write!(f, "-")?;
            }
            if self.has_any(Stroke(bit)) {
                write!(f, "{}", ch)?;
            }
            bit >>= 1;
        }

        Ok(())
    }
}

#[test]
fn stroke_roundtrip() {
    if let Err(_) = std::env::var("SDRILL_LONG_TESTS") {
        return;
    }

    for ch in 1u32 .. 0x800000 {
        let text = format!("{}", Stroke(ch));
        let orig = Stroke::from_text(&text).unwrap();
        if ch != orig.0 {
            println!("Mismatch: 0x{:x} -> {} -> 0x{:x}", ch, text, orig.0);
        }
        assert_eq!(ch, orig.0);
    }
}
