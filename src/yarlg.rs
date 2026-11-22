//! # yarlg
//!
//! _Blazingly fast Lexer generator._
//!
//! `yarlg` allows you to make Lexers that are built entirely at compile time.
//!
//! To achieve this `yarlg` does the following:
//!
//! + It turns Tokens defintions into a one single big [DFA](https://en.wikipedia.org/wiki/Deterministic_finite_automaton).
//! + It turns it into a 1D array that lives pre-compiled in memory where every possible state transitions is already pre-compiled.
//! + Character ranges are compiled into UTF-8 byte transitions. The lexer makes
//!   one O(1) lookup per byte, taking O(n) time for n bytes of input.
//!
//! See the [README](https://github.com/paoloanzn/yarlg/blob/master/README.md) for additional documentation and examples.

use std::fmt::{self};

/// A contiguous range of Unicode scalars, or a single Unicode scalar.
///
/// # Example
///
/// ```
/// use yarlg::CharacterRange as range;
///
/// // '0'...'9'
/// const DIGIT: range = range::new().with_start('0').with_end('9').build();
/// // ' '
/// const SPACE: range = range ::new().with_start(' ').build();
///
/// assert_eq!(DIGIT.to_string(), "'0'...'9'");
/// assert_eq!(SPACE.to_string(), "' '");
/// ```
#[derive(Default)]
pub struct CharacterRange {
    start: Option<char>,
    end: Option<char>,
}

/// A range can contain one, two or zero elements.
impl CharacterRange {
    /// Creates an empty range with neither `start` nor `end` set.
    pub const fn new() -> Self {
        return Self {
            start: None,
            end: None,
        };
    }

    /// Sets the first character of the range (inclusive).
    ///
    /// If no `end` is set, the range matches only this character.
    pub const fn with_start(mut self, start: char) -> Self {
        self.start = Some(start);
        self
    }

    /// Sets the last character of the range (inclusive).
    pub const fn with_end(mut self, end: char) -> Self {
        self.end = Some(end);
        self
    }

    /// Validates and returns the range.
    pub const fn build(self) -> Self {
        if let (Some(start), Some(end)) = (self.start, self.end) {
            if start > end {
                panic!("Error: `start` can't be greater than `end`");
            }
        }
        return self;
    }
}

impl fmt::Display for CharacterRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.start, self.end) {
            (Some(start), Some(end)) => {
                write!(f, "'{0}'...'{1}'", start, end)
            }
            (Some(start), None) => {
                write!(f, "'{0}'", start)
            }
            (None, Some(end)) => {
                write!(f, "'{0}'", end)
            }
            (None, None) => {
                write!(f, "<empty_range>")
            }
        }
    }
}

pub enum CharacterGroupType {
    /// {}
    ExactlyOne,
    /// {}+
    AtLeastOne,
    /// {}*
    ZeroOrMore,
}

/// A set of Unicode scalar ranges with an associated group type.
/// `{'0'...'9', 'a'...'z', 'A'...'Z'}+`.
///
/// # Example
///
/// ```
/// use yarlg::{CharacterRange as range, CharacterGroupType, group};
///
/// const LOWER: range = range::new().with_start('a').with_end('z').build();
/// const UNDERSCORE: range = range::new().with_start('_').build();
///
/// let word = group!(LOWER UNDERSCORE, CharacterGroupType::AtLeastOne);
/// assert_eq!(word.to_string(), "{'a'...'z', '_'}+");
/// ```
pub struct CharacterGroup<'a> {
    pub ranges: &'a [CharacterRange],
    pub group_type: CharacterGroupType,
}

/// Creates a [`CharacterGroup`] from one or more [`CharacterRange`]s
/// and a [`CharacterGroupType`].
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

/// A Token definition is two **ordered** lists of [`CharacterGroup`]s,
/// one for the characters of the token and one for its delimiters.
/// Plus a label to give the token a name.
pub struct Token<'a> {
    label: &'a str,
    groups: &'a [CharacterGroup<'a>],
    // TODO: evaluate if replacing [CharacterGroup] with one single group of ranges, CharacterGroup, is better.
    // The reason is that logically this will always be one single group.
    delimiters: &'a [CharacterGroup<'a>],
}

impl<'a> Token<'a> {
    /// Creates a new token definition.
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
    /// Builds all the possible transitions for the token.
    const fn build_transitions(
        &self,
        buf: &mut [(State, (char, char), State)],
    ) -> (usize, usize) {
        macro_rules! expand_range {
            ($start:expr, $end:expr, $arr:expr, $offset:expr, $state_start:expr, $state_end:expr) => {{
                $arr[$offset] = ($state_start, ($start, $end), $state_end);
                $offset += 1;
            }};
            ($char:expr, $arr:expr, $offset:expr, $state_start:expr, $state_end:expr) => {{
                $arr[$offset] = ($state_start, ($char, $char), $state_end);
                $offset += 1;
            }};
        }
        let mut num_transitions: usize = 0;
        let mut num_states: State = START_STATE;
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
                                start,
                                end,
                                buf,
                                num_transitions,
                                num_states,
                                num_states + 1
                            );
                        }
                        CharacterGroupType::AtLeastOne => {
                            expand_range!(
                                start,
                                end,
                                buf,
                                num_transitions,
                                num_states,
                                num_states + 1
                            );
                            expand_range!(
                                start,
                                end,
                                buf,
                                num_transitions,
                                num_states + 1,
                                num_states + 1
                            );
                        }
                        CharacterGroupType::ZeroOrMore => {
                            expand_range!(
                                start,
                                end,
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
                                start,
                                buf,
                                num_transitions,
                                num_states,
                                num_states + 1
                            );
                        }
                        CharacterGroupType::AtLeastOne => {
                            expand_range!(
                                start,
                                buf,
                                num_transitions,
                                num_states,
                                num_states + 1
                            );
                            expand_range!(
                                start,
                                buf,
                                num_transitions,
                                num_states + 1,
                                num_states + 1
                            );
                        }
                        CharacterGroupType::ZeroOrMore => {
                            expand_range!(
                                start,
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
                                end,
                                buf,
                                num_transitions,
                                num_states,
                                num_states + 1
                            );
                        }
                        CharacterGroupType::AtLeastOne => {
                            expand_range!(
                                end,
                                buf,
                                num_transitions,
                                num_states,
                                num_states + 1
                            );
                            expand_range!(
                                end,
                                buf,
                                num_transitions,
                                num_states + 1,
                                num_states + 1
                            );
                        }
                        CharacterGroupType::ZeroOrMore => {
                            expand_range!(
                                end,
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
                panic!(
                    "Error: delimiter groups cannot be ZeroOrMore, a delimiter is always required."
                );
            }
            let mut range_index: usize = 0;
            while range_index < delimiter.ranges.len() {
                let range = &delimiter.ranges[range_index];
                match (range.start, range.end) {
                    (Some(start), Some(end)) => match delimiter.group_type {
                        CharacterGroupType::ExactlyOne => {
                            expand_range!(
                                start,
                                end,
                                buf,
                                num_transitions,
                                num_states,
                                terminal_state
                            );
                        }
                        CharacterGroupType::AtLeastOne => {
                            expand_range!(
                                start,
                                end,
                                buf,
                                num_transitions,
                                num_states,
                                terminal_state
                            );
                            expand_range!(
                                start,
                                end,
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
                                start,
                                buf,
                                num_transitions,
                                num_states,
                                terminal_state
                            );
                        }
                        CharacterGroupType::AtLeastOne => {
                            expand_range!(
                                start,
                                buf,
                                num_transitions,
                                num_states,
                                terminal_state
                            );
                            expand_range!(
                                start,
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
                                end,
                                buf,
                                num_transitions,
                                num_states,
                                terminal_state
                            );
                        }
                        CharacterGroupType::AtLeastOne => {
                            expand_range!(
                                end,
                                buf,
                                num_transitions,
                                num_states,
                                terminal_state
                            );
                            expand_range!(
                                end,
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

// Some constants.
pub type State = isize;
const INVALID_STATE: isize = -1;
const START_STATE: State = 0;
const TRANSITIONS_PER_STATE: usize = 256;

// Computes the index of a given (State, char) key pair in the transition table.
#[doc(hidden)]
const fn compute_index(state: State, char: u8) -> usize {
    let offset: usize = state as usize * TRANSITIONS_PER_STATE as usize;
    return offset + char as usize;
}

// The table size is fixed and dependent from the amount of representable states
// needed for the lexer DFA. Each state needs to have is corresponding transition
// function result for every possible UTF-8 byte value, defined as `TRANSITIONS_PER_STATE`.
pub const fn compute_table_size(number_of_states: usize) -> usize {
    return number_of_states as usize * TRANSITIONS_PER_STATE as usize;
}

// Take the current state, one input character and lookup the resulting state in the table.
// This lookup is O(1).
#[doc(hidden)]
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

/// The compiled lexer lookup table and a list of terminal states.
///
/// Create it with [`table_init!`] and pass it to [`Lexer::run`].
pub struct TransitionTable<'a, const N: usize, const M: usize> {
    /// The table is flat 1-dim array.
    /// Each states gets its own computed offset: (state * TRANSITIONS_PER_STATE).
    pub table: [State; N],
    pub terminal_states: [(State, &'a str); M],
}

// Number of transitions `Token::build_transitions` writes for `groups`.
#[doc(hidden)]
const fn groups_transitions(groups: &[CharacterGroup]) -> usize {
    let mut num_transitions: usize = 0;
    let mut i: usize = 0;
    while i < groups.len() {
        let group = &groups[i];
        let mut j: usize = 0;
        while j < group.ranges.len() {
            let range = &group.ranges[j];
            let range_len: usize = match (range.start, range.end) {
                (Some(_), Some(_)) => 1,
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

// Default max number of states the tmp table can use.
// This **is not** the allocated space for the final Lexer table.
#[doc(hidden)]
pub const MAX_STATES: usize = 1024;

// Fails compilation if `table` has no room for `num_states` states.
//
// MAX_STATES defines an already generous allocated space for the tmp table,
// but we give the option to the user to optionally override the value manually
// when calling table_init!
#[doc(hidden)]
const fn check_table_space(table: &[State], num_states: usize) {
    if compute_table_size(num_states) > table.len() {
        panic!(
            "Error: the lexer needs more states than `max_states` allows. Increase it: table_init!(NAME, TOKENS, max_states = <bigger value>)"
        );
    }
}

// Counts the states needed by `tokens` by building them into a tmp table.
#[doc(hidden)]
pub const fn count_states<
    const T: usize,
    const MAX: usize,
    const SIZE: usize,
>(
    tokens: &[Token],
) -> usize {
    if tokens.len() > MAX {
        panic!(
            "Error: more tokens than `max_states` allows. Increase it: table_init!(NAME, TOKENS, max_states = <bigger value>)"
        );
    }
    let mut scratch: [State; SIZE] = [INVALID_STATE; SIZE];
    let mut scratch_terminal_states: [(State, &str); MAX] =
        [(INVALID_STATE, ""); MAX];
    return add_transitions_to_table::<T, MAX>(
        tokens,
        &mut scratch,
        &mut scratch_terminal_states,
    );
}

#[doc(hidden)]
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

/// Creates [`TransitionTable`], computed entirely at compile time.
///
/// # Example
///
/// ```ignore
/// table_init!(LEXER, TOKENS);
/// let tokens = yarlg::Lexer::run("42 foo", &LEXER);
/// ```
///
/// See [README](https://github.com/paoloanzn/yarlg/blob/master/README.md) for a complete lexer example.
///
/// # Panics
///
/// Since the table is a `const`, an invalid token definition fails compilation.
#[macro_export]
macro_rules! table_init {
    // @paoloanzn: this macro was written by claude after countless hours of pain.
    ($name:ident, $tokens:expr, max_states = $max:expr $(,)?) => {
        const $name: $crate::TransitionTable<
            'static,
            {
                $crate::compute_table_size($crate::count_states::<
                    { $crate::max_token_transitions($tokens) },
                    { $max },
                    { $crate::compute_table_size($max) },
                >($tokens))
            },
            { $tokens.len() },
        > = $crate::build_table::<
            {
                $crate::compute_table_size($crate::count_states::<
                    { $crate::max_token_transitions($tokens) },
                    { $max },
                    { $crate::compute_table_size($max) },
                >($tokens))
            },
            { $tokens.len() },
            { $crate::max_token_transitions($tokens) },
            { $max },
        >($tokens);
    };
    ($name:ident, $tokens:expr $(,)?) => {
        $crate::table_init!($name, $tokens, max_states = $crate::MAX_STATES);
    };
}

// Merges the `old` and `new` states into one single state that has the transitions of both.
// Terminal states are allocated in token order: the more negative one wins, so
// `yarlg` use a "last-declared-wins" approach for conflicting tokens.
//
// So the token definition order inside `const TOKENS: &[Token] = &[...];` before
// passing it to `table_init!(LEXER, TOKENS)` **matters**.
const fn merge(
    table: &mut [State],
    old: State,
    new: State,
    total_states: &mut usize,
    merged: &mut [(State, State, State)],
    num_merged: &mut usize,
) -> State {
    if new == INVALID_STATE {
        return old;
    }
    if old == INVALID_STATE || old == new {
        return new;
    }
    if old < 0 || new < 0 {
        return if new < old { new } else { old };
    }
    // Store unions as ordered pairs (smallest original state, remainder).
    // Thus overlapping loops reuse the same row, e.g. (A+B)+B == A+B.
    let mut a = (old, INVALID_STATE);
    let mut b = (new, INVALID_STATE);
    let mut i = 0;
    while i < *num_merged {
        let (head, tail, state) = merged[i];
        if old == state {
            a = (head, tail);
        }
        if new == state {
            b = (head, tail);
        }
        i += 1;
    }
    let (old, left, right) = if a.0 == b.0 {
        (a.0, a.1, b.1)
    } else if a.0 < b.0 {
        (a.0, a.1, new)
    } else {
        (b.0, old, b.1)
    };
    let new = merge(table, left, right, total_states, merged, num_merged);
    if new == INVALID_STATE {
        return old;
    }
    let mut i: usize = 0;
    while i < *num_merged {
        if merged[i].0 == old && merged[i].1 == new {
            return merged[i].2;
        }
        i += 1;
    }
    let state = *total_states as State;
    *total_states += 1;
    check_table_space(table, *total_states);
    merged[*num_merged] = (old, new, state);
    *num_merged += 1;
    return state;
}

// Write an inclusive scalar range as compact UTF-8 paths into the byte table.
const fn write_range(
    table: &mut [State],
    from: State,
    (first, last): (char, char),
    to: State,
    total_states: &mut usize,
    merged: &mut [(State, State, State)],
    num_merged: &mut usize,
) {
    let mut lo = first as u32;
    while lo <= last as u32 {
        // Never cross a UTF-8 length boundary or encode a surrogate.
        let limit = match lo {
            0..=0x7f => 0x80,
            0x80..=0x7ff => 0x800,
            0x800..=0xd7ff => 0xd800,
            0xd800..=0xdfff => {
                lo = 0xe000;
                continue;
            }
            0xe000..=0xffff => 0x10000,
            _ => 0x110000,
        };
        let end = if (last as u32) < limit {
            last as u32 + 1
        } else {
            limit
        };
        let mut first_bytes = [0; 4];
        let width = char::from_u32(lo)
            .unwrap()
            .encode_utf8(&mut first_bytes)
            .len();
        // Each continuation byte represents six bits. Consume the largest
        // aligned block with a common prefix, one varying byte, and zero
        // or more unrestricted 80..BF suffix bytes.
        let mut block = 1;
        let mut pivot = width - 1;
        while pivot > 0 && lo % (block * 64) == 0 && lo + block * 64 <= end {
            block *= 64;
            pivot -= 1;
        }
        let prefix_end = (lo | (block * 64 - 1)) + 1;
        let stop = if pivot != 0 && prefix_end < end {
            prefix_end
        } else {
            end
        };
        let next = lo + (stop - lo) / block * block;
        let mut last_bytes = [0; 4];
        char::from_u32(next - 1)
            .unwrap()
            .encode_utf8(&mut last_bytes);

        // Construct the suffix first. Only the first byte can collide with
        // an existing path; private continuation rows are already complete.
        let mut target = to;
        let mut i = width;
        while i > 0 {
            i -= 1;
            let source = if i == 0 {
                from
            } else {
                *total_states += 1;
                check_table_space(table, *total_states);
                (*total_states - 1) as State
            };
            let mut b = first_bytes[i] as usize;
            while b <= last_bytes[i] as usize {
                let index = compute_index(source, b as u8);
                table[index] = merge(
                    table,
                    table[index],
                    target,
                    total_states,
                    merged,
                    num_merged,
                );
                b += 1;
            }
            target = source;
        }
        lo = next;
    }
}

// Adds every token's transitions to `table` and remembers final states in `terminal_states`.
// Returns the total number of states used in `table`.
#[doc(hidden)]
const fn add_transitions_to_table<'a, const T: usize, const MAX: usize>(
    tokens: &'a [Token<'a>],
    table: &mut [State],
    terminal_states: &mut [(State, &'a str)],
) -> usize {
    let mut transition_buffer = [(0, ('\0', '\0'), 0); T];
    let mut merged: [(State, State, State); MAX] =
        [(INVALID_STATE, INVALID_STATE, INVALID_STATE); MAX];
    let mut num_merged: usize = 0;
    let mut total_states: usize = 1; // shared start row
    let mut i: usize = 0;
    while i < tokens.len() {
        let token = &tokens[i];
        let (num_states, num_transitions) =
            token.build_transitions(&mut transition_buffer);
        // A token's initial repetition must return to its own row, not to the
        // shared start (which also skips whitespace and starts other tokens).
        let private_start = match token.groups.first() {
            Some(group) => {
                matches!(group.group_type, CharacterGroupType::ZeroOrMore)
            }
            None => true,
        };
        let offset = (total_states - (!private_start as usize)) as State;
        total_states = offset as usize + num_states;
        check_table_space(table, total_states);
        let final_state = total_states - 1;
        let mut j = 0;
        while j < num_transitions {
            let (from, range, to) = transition_buffer[j];
            let target = if to < 0 { to - offset } else { to + offset };
            write_range(
                table,
                if from == START_STATE && !private_start {
                    START_STATE
                } else {
                    from + offset
                },
                range,
                target,
                &mut total_states,
                &mut merged,
                &mut num_merged,
            );
            j += 1;
        }
        let mut byte = 0;
        while private_start && byte < TRANSITIONS_PER_STATE {
            let new = table[compute_index(offset, byte as u8)];
            table[byte] = merge(
                table,
                table[byte],
                new,
                &mut total_states,
                &mut merged,
                &mut num_merged,
            );
            byte += 1;
        }
        terminal_states[i] = (-(final_state as isize), token.label);
        i += 1;
    }
    // Fill merged rows only after all original rows (including loops) are
    // complete. A pair's operands predate it; newly discovered pairs are
    // appended and filled in this same pass.
    let mut i = 0;
    while i < num_merged {
        let (old, new, state) = merged[i];
        let mut c = 0;
        while c < TRANSITIONS_PER_STATE {
            let old_end = table[compute_index(old, c as u8)];
            let new_end = table[compute_index(new, c as u8)];
            table[compute_index(state, c as u8)] = merge(
                table,
                old_end,
                new_end,
                &mut total_states,
                &mut merged,
                &mut num_merged,
            );
            c += 1;
        }
        i += 1;
    }
    return total_states;
}

/// Builds the [`TransitionTable`] for `tokens`.
///
/// **Do not** call this function directly, instead use [`table_init!`].
pub const fn build_table<
    'a,
    const N: usize,
    const M: usize,
    const T: usize,
    const MAX: usize,
>(
    tokens: &'a [Token<'a>],
) -> TransitionTable<'a, N, M> {
    if tokens.len() != M {
        panic!("Error: `M` must be equal to the number of tokens.");
    }
    let mut table: [State; N] = [INVALID_STATE; N];
    let mut terminal_states: [(State, &'a str); M] = [(INVALID_STATE, ""); M];
    add_transitions_to_table::<T, MAX>(
        tokens,
        &mut table,
        &mut terminal_states,
    );
    // `START_STATE` is shared by all tokens, so any of the below specified
    // leading chars are always added as a recursive self transition to `START_STATE`.
    let leading_skip: [u8; 4] = [b' ', b'\t', b'\n', b'\r'];
    let mut i: usize = 0;
    while i < leading_skip.len() {
        let index = compute_index(START_STATE, leading_skip[i]);
        if table[index] == INVALID_STATE {
            table[index] = START_STATE;
        }
        i += 1;
    }
    return TransitionTable {
        table: table,
        terminal_states: terminal_states,
    };
}

pub struct Lexer {}

impl Lexer {
    /// Returns the tokens in input order as `(label, text)` pairs, where
    /// `label` is the [`Token`] label and `text` is the matched text
    /// without its delimiter.
    ///
    /// # Example
    ///
    /// ```
    /// use yarlg::{CharacterRange as range, CharacterGroupType, Token, group, table_init};
    ///
    /// const DIGIT: range = range::new().with_start('0').with_end('9').build();
    /// const COMMA: range = range::new().with_start(',').build();
    ///
    /// const TOKENS: &[Token] = &[
    /// Token::new("NUMBER",
    ///     &[group!(DIGIT, CharacterGroupType::AtLeastOne)],
    ///     &[group!(COMMA, CharacterGroupType::ExactlyOne)],
    /// )
    /// ];
    ///
    /// table_init!(LEXER, TOKENS);
    ///
    /// let tokens = yarlg::Lexer::run("1,22,", &LEXER).unwrap();
    /// assert_eq!(tokens, vec![("NUMBER", "1".into()), ("NUMBER", "22".into())]);
    ///
    /// // The last token is not followed by ',' and NUMBER has no '\0' delimiter.
    /// assert!(yarlg::Lexer::run("1,22", &LEXER).is_err());
    /// ```
    pub fn run<'a, const N: usize, const M: usize>(
        input: &str,
        table: &TransitionTable<'a, N, M>,
    ) -> Result<Vec<(&'a str, String)>, String> {
        let mut token_start = 0;
        let mut scalar_start = 0;
        let mut tokens_found: Vec<(&'a str, String)> = vec![];
        let mut state: State = START_STATE;
        // IMPORTANT: A trailing '\0' terminates the last token, so it must be a delimiter of every token.
        for (offset, byte) in
            input.bytes().chain(std::iter::once(0)).enumerate()
        {
            if byte == 0 && state == START_STATE {
                break;
            }
            // Remember the whole delimiter (or unexpected scalar), even when
            // its final byte triggers the transition.
            if input.is_char_boundary(offset) {
                scalar_start = offset;
            }
            state = transition(state, byte, &table.table);
            match state {
                INVALID_STATE => {
                    return Err(format!(
                        "Error: input string is not a valid string; unexpected character `{}`",
                        input[scalar_start..].chars().next().unwrap_or('\0')
                    ));
                }
                _ if state < -1 => {
                    let label = match table
                        .terminal_states
                        .iter()
                        .find(|(terminal_state, _)| *terminal_state == state)
                    {
                        Some((_, label)) => *label,
                        None => {
                            return Err(format!(
                                "Error: no token label found for terminal state `{}`",
                                state
                            ));
                        }
                    };
                    tokens_found.push((
                        label,
                        input[token_start..scalar_start].to_owned(),
                    ));
                    state = START_STATE;
                    token_start = offset + 1;
                }
                START_STATE => token_start = offset + 1,
                _ if offset == input.len() => {
                    return Err(
                        "Error: expected a delimiter at end of input".into()
                    );
                }
                _ => {}
            }
        }
        return Ok(tokens_found);
    }
}
