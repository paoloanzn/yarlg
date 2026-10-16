// yarlg.rs: Yet another Rust lexer generator.

use core::num;
use std::{
    fmt::{self, Debug, Error},
    num::FpCategory::Nan,
};

// Ordinal continguos range of ASCII characters.
// '0'...'9', 'a'...'z'
#[derive(Debug, Default)]
pub struct AsciiCharacterRange {
    start: Option<u8>,
    end: Option<u8>,
}

impl AsciiCharacterRange {
    pub fn new() -> Self {
        return Self {
            ..Default::default()
        };
    }

    pub fn with_start(mut self, start: u8) -> Self {
        self.start = Some(start);
        self
    }

    pub fn with_end(mut self, end: u8) -> Self {
        self.end = Some(end);
        self
    }

    pub fn build(self) -> Result<Self, String> {
        match (self.start, self.end) {
            (Some(start), Some(end)) => {
                if end > TRANSITIONS_PER_STATE {
                    return Err(format!(
                        "Error: `end`({}) is over the max allowed value({})",
                        end, TRANSITIONS_PER_STATE
                    ));
                }
                if start > TRANSITIONS_PER_STATE {
                    return Err(format!(
                        "Error: `start`({}) is over the max allowed value({})",
                        start, TRANSITIONS_PER_STATE
                    ));
                }
                if start > end {
                    return Err(format!(
                        "Error: `start`({}) can't be greater than `end`({})",
                        start, end
                    ));
                }
                return Ok(self);
            }
            (Some(start), None) => {
                if start > TRANSITIONS_PER_STATE {
                    return Err(format!(
                        "Error: `start`({}) is over the max allowed value({})",
                        start, TRANSITIONS_PER_STATE
                    ));
                }
                return Ok(self);
            }
            (None, Some(end)) => {
                if end > TRANSITIONS_PER_STATE {
                    return Err(format!(
                        "Error: `end`({}) is over the max allowed value({})",
                        end, TRANSITIONS_PER_STATE
                    ));
                }
                return Ok(self);
            }
            (None, None) => return Ok(self),
        }
    }
}

impl fmt::Display for AsciiCharacterRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.start, self.end) {
            (Some(start), Some(end)) => {
                write!(f, "{0}...{1}", start as char, end as char)
            }
            (Some(start), None) => {
                write!(f, "{0}", start as char)
            }
            (None, Some(end)) => {
                write!(f, "{0}", end as char)
            }
            (None, None) => {
                write!(f, "<empty_range>")
            }
        }
    }
}

//
enum CharacterGroupType {
    ExactlyOne, // {}
    AtLeastOne, // {}+
    ZeroOrMore, // {}*
}

// Set of ranges of ASCII characters.
// {0..9, a...z, A...Z}+
struct CharacterGroup<'a> {
    elements: &'a [AsciiCharacterRange],
    group_type: CharacterGroupType,
}

struct Token<'a> {
    label: &'a str,
    ranges: [CharacterGroup<'a>],
}

type State = usize;
const TRANSITIONS_PER_STATE: u8 = 128;

// Used compute the index of a given (State, char) key pair in the transition table.
fn compute_index(state: State, char: u8) -> usize {
    let offset: usize = state * TRANSITIONS_PER_STATE as usize;
    return offset + char as usize;
}

// The table size is fixed and dependent from the amount of representable states
// needed for the lexer DFA.
// Each state needs to have is corresponding transition function result for every
// possible ASCII character value, defined as `TRANSITIONS_PER_STATE`.
fn compute_table_size(number_of_states: i32) -> usize {
    return (number_of_states as usize * TRANSITIONS_PER_STATE as usize);
}

fn transition(state_in: State, char: u8, table: &Vec<State>) -> State {
    let index: usize = compute_index(state_in, char);
    return match table.get(index) {
        Some(state) => *state,
        None => {
            panic!(
                "Error: transition not found for ({}, {}) not found.",
                state_in, char
            );
        }
    };
}
