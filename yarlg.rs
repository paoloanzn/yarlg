// yarlg.rs: Yet another Rust lexer generator.

use std::{
    fmt::{self, Debug},
    num,
};

// Ordinal continguos range of ASCII characters.
// '0'...'9', 'a'...'z'
#[derive(Debug, Default)]
pub struct AsciiCharacterRange {
    start: Option<u8>,
    end: Option<u8>,
}

// A range can contain 1, 2 or zero elements.
// You can create a range like this:
// `yarlg::AsciiCharacterRange::new()
//      .with_start('0' as u8).with_end('9' as u8)
//      .build()`
//
// `start` can never be greater than `end` and neither
// of them can be ge than `TRANSITIONS_PER_STATE`.
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
                if end >= TRANSITIONS_PER_STATE {
                    return Err(format!(
                        "Error: `end`({}) is over the max allowed value({})",
                        end, TRANSITIONS_PER_STATE
                    ));
                }
                if start >= TRANSITIONS_PER_STATE {
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
                if start >= TRANSITIONS_PER_STATE {
                    return Err(format!(
                        "Error: `start`({}) is over the max allowed value({})",
                        start, TRANSITIONS_PER_STATE
                    ));
                }
                return Ok(self);
            }
            (None, Some(end)) => {
                if end >= TRANSITIONS_PER_STATE {
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
                write!(f, "'{0}'...'{1}'", start as char, end as char)
            }
            (Some(start), None) => {
                write!(f, "'{0}'", start as char)
            }
            (None, Some(end)) => {
                write!(f, "'{0}'", end as char)
            }
            (None, None) => {
                write!(f, "<empty_range>")
            }
        }
    }
}

pub enum CharacterGroupType {
    ExactlyOne, // {}
    AtLeastOne, // {}+
    ZeroOrMore, // {}*
}

// Set of ranges of ASCII characters.
// {0..9, a...z, A...Z}+
pub struct CharacterGroup<'a> {
    pub ranges: &'a [AsciiCharacterRange],
    pub group_type: CharacterGroupType,
}

// Creates a new CharacterGroup by passing ranges and a CharacterGroupType.
#[macro_export]
macro_rules! group {
    ($($arg:expr)+, $type:expr $(,)?) => {
       $crate::CharacterGroup {
            ranges: &[$($arg ,)+],
            group_type: $type
       };
    };
}

impl<'a> CharacterGroup<'a> {
    fn format_ranges(&self) -> String {
        use std::fmt::Write;
        let mut out = String::from("");
        for (i, item) in self.ranges.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            match write!(&mut out, "{}", item) {
                Ok(res) => res,
                Err(error) => {
                    panic!("An unexpected error occured: {}", error);
                }
            }
        }
        return out;
    }
}

// Pretty-print for CharacterGroup:
// `{'a'...'Z', 'A'...'Z`}*`
impl<'a> fmt::Display for CharacterGroup<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let group_type_symbol: char = match self.group_type {
            CharacterGroupType::AtLeastOne => '+',
            CharacterGroupType::ExactlyOne => '\0',
            CharacterGroupType::ZeroOrMore => '*',
        };
        write!(f, "{{{}}}{}", self.format_ranges(), group_type_symbol)
    }
}

struct Token<'a> {
    label: &'a str,
    groups: &'a [CharacterGroup<'a>],
    // TODO: evaluate if replacing [CharacterGroup] with one single group of ranges, CharacterGroup, is better.
    // The reason is that logically this will always be one single group.
    delimiters: &'a [CharacterGroup<'a>],
}

impl<'a> Token<'a> {
    pub fn new(
        label: &'a str,
        groups: &'a [CharacterGroup],
        delimiters: &'a [CharacterGroup],
    ) -> Self {
        return Self {
            label: label,
            groups: groups,
            delimiters: delimiters,
        };
    }

    fn build_transitions(
        &self,
        buf: &mut [(State, char, State)],
    ) -> (usize, usize) {
        macro_rules! expand_range {
            ($start:expr, $end:expr, $arr:expr, $offset:expr, $state_start:expr, $state_end:expr) => {{
                for value in $start..=$end {
                    $arr[$offset] = ($state_start, value, $state_end);
                    $offset += 1;
                }
            }};
            ($char:expr, $arr:expr, $offset:expr, $state_start:expr, $state_end:expr) => {{
                $arr[$offset] = ($state_start, $char, $state_end);
                $offset += 1;
            }};
        }
        let mut num_transitions: usize = 0;
        let mut num_states: usize = 0;
        for group in self.groups.iter() {
            for range in group.ranges.iter() {
                match (range.start, range.end) {
                    (Some(start), Some(end)) => match group.group_type {
                        CharacterGroupType::ExactlyOne => {
                            expand_range!(
                                start as char,
                                end as char,
                                buf,
                                num_transitions,
                                num_states,
                                num_states + 1
                            );
                        }
                        CharacterGroupType::AtLeastOne => {
                            expand_range!(
                                start as char,
                                end as char,
                                buf,
                                num_transitions,
                                num_states,
                                num_states + 1
                            );
                            expand_range!(
                                start as char,
                                end as char,
                                buf,
                                num_transitions,
                                num_states + 1,
                                num_states + 1
                            );
                        }
                        CharacterGroupType::ZeroOrMore => {
                            expand_range!(
                                start as char,
                                end as char,
                                buf,
                                num_transitions,
                                num_states,
                                num_states
                            );
                        }
                    },
                    (Some(start), None) => match group.group_type {
                        CharacterGroupType::ExactlyOne => {
                            expand_range!(
                                start as char,
                                buf,
                                num_transitions,
                                num_states,
                                num_states + 1
                            );
                        }
                        CharacterGroupType::AtLeastOne => {
                            expand_range!(
                                start as char,
                                buf,
                                num_transitions,
                                num_states,
                                num_states + 1
                            );
                            expand_range!(
                                start as char,
                                buf,
                                num_transitions,
                                num_states + 1,
                                num_states + 1
                            );
                        }
                        CharacterGroupType::ZeroOrMore => {
                            expand_range!(
                                start as char,
                                buf,
                                num_transitions,
                                num_states,
                                num_states
                            );
                        }
                    },
                    (None, Some(end)) => match group.group_type {
                        CharacterGroupType::ExactlyOne => {
                            expand_range!(
                                end as char,
                                buf,
                                num_transitions,
                                num_states,
                                num_states + 1
                            );
                        }
                        CharacterGroupType::AtLeastOne => {
                            expand_range!(
                                end as char,
                                buf,
                                num_transitions,
                                num_states,
                                num_states + 1
                            );
                            expand_range!(
                                end as char,
                                buf,
                                num_transitions,
                                num_states + 1,
                                num_states + 1
                            );
                        }
                        CharacterGroupType::ZeroOrMore => {
                            expand_range!(
                                end as char,
                                buf,
                                num_transitions,
                                num_states,
                                num_states
                            );
                        }
                    },
                    (None, None) => {}
                }
            }
            match group.group_type {
                CharacterGroupType::AtLeastOne => num_states += 1,
                CharacterGroupType::ExactlyOne => num_states += 1,
                CharacterGroupType::ZeroOrMore => num_states += 0,
            }
        }
        let final_state: State = num_states + 1;
        for delimiter in self.delimiters.iter() {
            if let CharacterGroupType::ZeroOrMore = delimiter.group_type {
                panic!("Error: delimiter groups cannot be ZeroOrMore, a delimiter is always required.");
            }
            for range in delimiter.ranges.iter() {
                match (range.start, range.end) {
                    (Some(start), Some(end)) => match delimiter.group_type {
                        CharacterGroupType::ExactlyOne => {
                            expand_range!(
                                start as char,
                                end as char,
                                buf,
                                num_transitions,
                                num_states,
                                final_state
                            );
                        }
                        CharacterGroupType::AtLeastOne => {
                            expand_range!(
                                start as char,
                                end as char,
                                buf,
                                num_transitions,
                                num_states,
                                final_state
                            );
                            expand_range!(
                                start as char,
                                end as char,
                                buf,
                                num_transitions,
                                final_state,
                                final_state
                            );
                        }
                        CharacterGroupType::ZeroOrMore => unreachable!(),
                    },
                    (Some(start), None) => match delimiter.group_type {
                        CharacterGroupType::ExactlyOne => {
                            expand_range!(
                                start as char,
                                buf,
                                num_transitions,
                                num_states,
                                final_state
                            );
                        }
                        CharacterGroupType::AtLeastOne => {
                            expand_range!(
                                start as char,
                                buf,
                                num_transitions,
                                num_states,
                                final_state
                            );
                            expand_range!(
                                start as char,
                                buf,
                                num_transitions,
                                final_state,
                                final_state
                            );
                        }
                        CharacterGroupType::ZeroOrMore => unreachable!(),
                    },
                    (None, Some(end)) => match delimiter.group_type {
                        CharacterGroupType::ExactlyOne => {
                            expand_range!(
                                end as char,
                                buf,
                                num_transitions,
                                num_states,
                                final_state
                            );
                        }
                        CharacterGroupType::AtLeastOne => {
                            expand_range!(
                                end as char,
                                buf,
                                num_transitions,
                                num_states,
                                final_state
                            );
                            expand_range!(
                                end as char,
                                buf,
                                num_transitions,
                                final_state,
                                final_state
                            );
                        }
                        CharacterGroupType::ZeroOrMore => unreachable!(),
                    },
                    (None, None) => {}
                }
            }
        }
        return (num_transitions, num_states + 2);
    }
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
