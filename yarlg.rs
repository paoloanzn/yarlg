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
    pub const fn new() -> Self {
        return Self {
            start: None,
            end: None,
        };
    }

    pub const fn with_start(mut self, start: u8) -> Self {
        self.start = Some(start);
        self
    }

    pub const fn with_end(mut self, end: u8) -> Self {
        self.end = Some(end);
        self
    }

    pub const fn build(self) -> Self {
        if let Some(start) = self.start {
            if start >= TRANSITIONS_PER_STATE {
                panic!("Error: `start` is over the max allowed value (127)");
            }
        }
        if let Some(end) = self.end {
            if end >= TRANSITIONS_PER_STATE {
                panic!("Error: `end` is over the max allowed value (127)");
            }
        }
        if let (Some(start), Some(end)) = (self.start, self.end) {
            if start > end {
                panic!("Error: `start` can't be greater than `end`");
            }
        }
        return self;
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
       }
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

pub struct Token<'a> {
    label: &'a str,
    groups: &'a [CharacterGroup<'a>],
    // TODO: evaluate if replacing [CharacterGroup] with one single group of ranges, CharacterGroup, is better.
    // The reason is that logically this will always be one single group.
    delimiters: &'a [CharacterGroup<'a>],
}

impl<'a> Token<'a> {
    pub const fn new(
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

    const fn build_transitions(
        &self,
        buf: &mut [(State, char, State)],
    ) -> (usize, usize) {
        macro_rules! expand_range {
            ($start:expr, $end:expr, $arr:expr, $offset:expr, $state_start:expr, $state_end:expr) => {{
                let mut value: u8 = $start as u8;
                while value <= $end as u8 {
                    $arr[$offset] = ($state_start, value as char, $state_end);
                    $offset += 1;
                    value += 1;
                }
            }};
            ($char:expr, $arr:expr, $offset:expr, $state_start:expr, $state_end:expr) => {{
                $arr[$offset] = ($state_start, $char, $state_end);
                $offset += 1;
            }};
        }
        let mut num_transitions: usize = 0;
        let mut num_states: State = 0;
        let mut group_index: usize = 0;
        while group_index < self.groups.len() {
            let group = &self.groups[group_index];
            let mut range_index: usize = 0;
            while range_index < group.ranges.len() {
                let range = &group.ranges[range_index];
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
                range_index += 1;
            }
            match group.group_type {
                CharacterGroupType::AtLeastOne => num_states += 1,
                CharacterGroupType::ExactlyOne => num_states += 1,
                CharacterGroupType::ZeroOrMore => num_states += 0,
            }
            group_index += 1;
        }
        let final_state: State = num_states + 1;
        // Transitions into the final state are stored as its negative counterpart
        // to flag it as terminal. Its row in the table is still indexed by `final_state`.
        let terminal_state: State = -final_state;
        let mut delimiter_index: usize = 0;
        while delimiter_index < self.delimiters.len() {
            let delimiter = &self.delimiters[delimiter_index];
            if let CharacterGroupType::ZeroOrMore = delimiter.group_type {
                panic!("Error: delimiter groups cannot be ZeroOrMore, a delimiter is always required.");
            }
            let mut range_index: usize = 0;
            while range_index < delimiter.ranges.len() {
                let range = &delimiter.ranges[range_index];
                match (range.start, range.end) {
                    (Some(start), Some(end)) => match delimiter.group_type {
                        CharacterGroupType::ExactlyOne => {
                            expand_range!(
                                start as char,
                                end as char,
                                buf,
                                num_transitions,
                                num_states,
                                terminal_state
                            );
                        }
                        CharacterGroupType::AtLeastOne => {
                            expand_range!(
                                start as char,
                                end as char,
                                buf,
                                num_transitions,
                                num_states,
                                terminal_state
                            );
                            expand_range!(
                                start as char,
                                end as char,
                                buf,
                                num_transitions,
                                final_state,
                                terminal_state
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
                                terminal_state
                            );
                        }
                        CharacterGroupType::AtLeastOne => {
                            expand_range!(
                                start as char,
                                buf,
                                num_transitions,
                                num_states,
                                terminal_state
                            );
                            expand_range!(
                                start as char,
                                buf,
                                num_transitions,
                                final_state,
                                terminal_state
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
                                terminal_state
                            );
                        }
                        CharacterGroupType::AtLeastOne => {
                            expand_range!(
                                end as char,
                                buf,
                                num_transitions,
                                num_states,
                                terminal_state
                            );
                            expand_range!(
                                end as char,
                                buf,
                                num_transitions,
                                final_state,
                                terminal_state
                            );
                        }
                        CharacterGroupType::ZeroOrMore => unreachable!(),
                    },
                    (None, None) => {}
                }
                range_index += 1;
            }
            delimiter_index += 1;
        }
        return ((num_states + 2) as usize, num_transitions);
    }
}

pub type State = isize;
const INVALID_STATE: isize = -1;
const TRANSITIONS_PER_STATE: u8 = 128;

// Used compute the index of a given (State, char) key pair in the transition table.
const fn compute_index(state: State, char: u8) -> usize {
    let offset: usize = state as usize * TRANSITIONS_PER_STATE as usize;
    return offset + char as usize;
}

// The table size is fixed and dependent from the amount of representable states
// needed for the lexer DFA.
// Each state needs to have is corresponding transition function result for every
// possible ASCII character value, defined as `TRANSITIONS_PER_STATE`.
pub const fn compute_table_size(number_of_states: usize) -> usize {
    return number_of_states as usize * TRANSITIONS_PER_STATE as usize;
}

fn transition(state_in: State, char: u8, table: &[State]) -> State {
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

// The lexer DFA.
// `N` is the table size (states * `TRANSITIONS_PER_STATE`).
// `M` the number of tokens.
//
// Terminal states are stored as negative values: a transition into -4 means
// "token accepted"; its outgoing transitions are still at row 4.
pub struct TransitionTable<'a, const N: usize, const M: usize> {
    pub table: [State; N],
    pub terminal_states: [(State, &'a str); M],
}

// Number of local states `Token::build_transitions` produces for `token`:
const fn token_states(token: &Token) -> usize {
    let mut num_states: usize = 2;
    let mut i: usize = 0;
    while i < token.groups.len() {
        match token.groups[i].group_type {
            CharacterGroupType::AtLeastOne => num_states += 1,
            CharacterGroupType::ExactlyOne => num_states += 1,
            CharacterGroupType::ZeroOrMore => num_states += 0,
        }
        i += 1;
    }
    return num_states;
}

// Number of transitions `Token::build_transitions` writes for `groups`.
const fn groups_transitions(groups: &[CharacterGroup]) -> usize {
    let mut num_transitions: usize = 0;
    let mut i: usize = 0;
    while i < groups.len() {
        let group = &groups[i];
        let mut j: usize = 0;
        while j < group.ranges.len() {
            let range = &group.ranges[j];
            let range_len: usize = match (range.start, range.end) {
                (Some(start), Some(end)) => (end - start) as usize + 1,
                (Some(_), None) | (None, Some(_)) => 1,
                (None, None) => 0,
            };
            match group.group_type {
                CharacterGroupType::AtLeastOne => {
                    num_transitions += 2 * range_len
                }
                CharacterGroupType::ExactlyOne => num_transitions += range_len,
                CharacterGroupType::ZeroOrMore => num_transitions += range_len,
            }
            j += 1;
        }
        i += 1;
    }
    return num_transitions;
}

// Total number of states of the DFA.
// The start state 0 is shared by every token.
pub const fn count_states(tokens: &[Token]) -> usize {
    let mut total_states: usize = 0;
    let mut i: usize = 0;
    while i < tokens.len() {
        total_states += token_states(&tokens[i]);
        if i > 0 {
            total_states -= 1;
        }
        i += 1;
    }
    return total_states;
}

pub const fn max_token_transitions(tokens: &[Token]) -> usize {
    let mut max: usize = 0;
    let mut i: usize = 0;
    while i < tokens.len() {
        let token = &tokens[i];
        let num_transitions = groups_transitions(token.groups)
            + groups_transitions(token.delimiters);
        if num_transitions > max {
            max = num_transitions;
        }
        i += 1;
    }
    return max;
}

// Defines a `const` TransitionTable called `$name` built from `$tokens`,
// which must be a `const` of type `&[Token]`.
// `table_init!(LEXER, TOKENS);`
#[macro_export]
macro_rules! table_init {
    // @paoloanzn: this macro was written by claude after countless hours of pain.
    ($name:ident, $tokens:expr $(,)?) => {
        const $name: $crate::TransitionTable<
            'static,
            { $crate::compute_table_size($crate::count_states($tokens)) },
            { $tokens.len() },
        > = $crate::build_table::<
            { $crate::compute_table_size($crate::count_states($tokens)) },
            { $tokens.len() },
            { $crate::max_token_transitions($tokens) },
        >($tokens);
    };
}

// `N`: table size, `M`: number of tokens, `T`: transition buffer size.
// Use `table_init!` instead of calling this directly.
pub const fn build_table<'a, const N: usize, const M: usize, const T: usize>(
    tokens: &'a [Token<'a>],
) -> TransitionTable<'a, N, M> {
    macro_rules! set_transitions {
        ($table:expr, $buf:expr, $num_states:expr, $token_states:expr, $num_transitions:expr) => {
            {
                // Number of global states used before this token.
                // Only the start state 0 is shared since it's unique for the whole DFA.
                let offset = ($num_states - $token_states) as isize;
                let mut i: usize = 0;
                while i < $num_transitions {
                    let (mut state_start, c, mut state_end) = $buf[i];
                    if state_start != 0 {
                        state_start += offset;
                    }
                    // Terminal states are negative: shift their absolute value.
                    if state_end < 0 {
                        state_end -= offset;
                    } else {
                        state_end += offset;
                    }
                    let index = compute_index(state_start, c as u8);
                    if $table[index] != INVALID_STATE {
                        let src = compute_index($table[index].abs(), 0);
                        let dest = compute_index(state_end.abs(), 0);
                        let mut k: usize = 0;
                        while k < TRANSITIONS_PER_STATE as usize {
                            $table[dest + k] = $table[src + k];
                            k += 1;
                        }
                    }
                    $table[index] = state_end;
                    i += 1;
                }
           }
        };
    }
    if tokens.len() != M {
        panic!("Error: `M` must be equal to the number of tokens.");
    }
    let mut table: [State; N] = [INVALID_STATE; N];
    let mut terminal_states: [(State, &'a str); M] = [(INVALID_STATE, ""); M];
    let mut transition_buffer: [(State, char, State); T] =
        [(0, 0 as char, 0); T];
    let mut total_states: usize = 0;
    let mut i: usize = 0;
    while i < tokens.len() {
        let token = &tokens[i];
        let (num_states, num_transitions) =
            token.build_transitions(&mut transition_buffer);
        total_states += num_states;
        if i > 0 {
            total_states -= 1;
        }
        set_transitions!(
            table,
            transition_buffer,
            total_states,
            num_states,
            num_transitions
        );
        terminal_states[i] = (-((total_states - 1) as isize), token.label);
        i += 1;
    }
    return TransitionTable {
        table: table,
        terminal_states: terminal_states,
    };
}
