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
//! + The resulting lexer process every character of the input string by doing a direct look up on that array.
//! 
//! As a result `yarlg` turns every single character processing into an O(1) operation and the complete input processing
//! into an O(n) operation where n is the lenght of the input.
//!
//! See the [README](https://github.com/paoloanzn/yarlg/blob/master/README.md) for additional documentation and examples.

use std::fmt::{self};

/// A contiguous range of ASCII characters, or a single ASCII character.
///
/// # Example
///
/// ```
/// use yarlg::AsciiCharacterRange as range;
///
/// // '0'...'9'
/// const DIGIT: range = range::new().with_start('0' as u8).with_end('9' as u8).build();
/// // ' '
/// const SPACE: range = range ::new().with_start(' ' as u8).build();
///
/// assert_eq!(DIGIT.to_string(), "'0'...'9'");
/// assert_eq!(SPACE.to_string(), "' '");
/// ```
#[derive(Default)]
pub struct AsciiCharacterRange {
    start: Option<u8>,
    end: Option<u8>,
}

/// A range can contain one, two or zero elements.
impl AsciiCharacterRange {
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
    pub const fn with_start(mut self, start: u8) -> Self {
        self.start = Some(start);
        self
    }

    /// Sets the last character of the range (inclusive).
    pub const fn with_end(mut self, end: u8) -> Self {
        self.end = Some(end);
        self
    }

    /// Validates and returns the range.
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
    /// {}
    ExactlyOne,
    /// {}+
    AtLeastOne,
    /// {}*
    ZeroOrMore,
}

/// A set of ASCII character ranges with an associated group type.
/// `{'0'...'9', 'a'...'z', 'A'...'Z'}+`.
///
/// # Example
///
/// ```
/// use yarlg::{AsciiCharacterRange as range, CharacterGroupType, group};
///
/// const LOWER: range = range::new().with_start('a' as u8).with_end('z' as u8).build();
/// const UNDERSCORE: range = range::new().with_start(b'_').build();
///
/// let word = group!(LOWER UNDERSCORE, CharacterGroupType::AtLeastOne);
/// assert_eq!(word.to_string(), "{'a'...'z', '_'}+");
/// ```
pub struct CharacterGroup<'a> {
    pub ranges: &'a [AsciiCharacterRange],
    pub group_type: CharacterGroupType,
}

/// Creates a [`CharacterGroup`] from one or more [`AsciiCharacterRange`]s
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

// Some constants.
pub type State = isize;
const INVALID_STATE: isize = -1;
const START_STATE: State = 0;
const TRANSITIONS_PER_STATE: u8 = 128;

// Computes the index of a given (State, char) key pair in the transition table.
#[doc(hidden)]
const fn compute_index(state: State, char: u8) -> usize {
    let offset: usize = state as usize * TRANSITIONS_PER_STATE as usize;
    return offset + char as usize;
}

// The table size is fixed and dependent from the amount of representable states
// needed for the lexer DFA. Each state needs to have is corresponding transition 
// function result for every possible ASCII character value, defined as `TRANSITIONS_PER_STATE`.
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

// Number of local states `Token::build_transitions` produces for `token`.
#[doc(hidden)]
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

#[doc(hidden)]
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

/// Builds the [`TransitionTable`] for `tokens`.
///
/// **Do not** call this function directly, instead use [`table_init!`].
pub const fn build_table<'a, const N: usize, const M: usize, const T: usize>(
    tokens: &'a [Token<'a>],
) -> TransitionTable<'a, N, M> {
    macro_rules! set_transitions {
        ($table:expr, $buf:expr, $num_states:expr, $token_states:expr, $num_transitions:expr) => {{
            // Number of global states used before this token.
            // Only `START_STATE` is shared since it's unique for the whole DFA.
            let offset = ($num_states - $token_states) as isize;
            let mut i: usize = 0;
            while i < $num_transitions {
                let (mut state_start, c, mut state_end) = $buf[i];
                if state_start != START_STATE {
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
        }};
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

pub struct Lexer {}

// One single token can't be longer then this amount of chars.
// TODO: make this overridable by the user.
const TOKEN_MAX_LEN: usize = 4096;

impl Lexer {
    /// Returns the tokens in input order as `(label, text)` pairs, where
    /// `label` is the [`Token`] label and `text` is the matched text
    /// without its delimiter.
    ///
    /// # Example
    ///
    /// ```
    /// use yarlg::{AsciiCharacterRange as range, CharacterGroupType, Token, group, table_init};
    ///
    /// const DIGIT: range = range::new().with_start('0' as u8).with_end('9' as u8).build();
    /// const COMMA: range = range::new().with_start(',' as u8).build();
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
        let mut token_buf: [char; TOKEN_MAX_LEN + 1] =
            ['\0'; TOKEN_MAX_LEN + 1];
        let mut token_len: usize = 0;
        let mut tokens_found: Vec<(&'a str, String)> = vec![];
        let mut state: State = START_STATE;
        // IMPORTANT: A trailing '\0' terminates the last token, so it must be a delimiter of every token.
        for c in input.chars().chain(std::iter::once('\0')) {
            // Input already ended on a delimiter: nothing left to terminate.
            if c == '\0' && state == START_STATE {
                break;
            }
            state = transition(state, c as u8, &table.table);
            match state {
                INVALID_STATE => {
                    return Err(format!(
                        "Error: input string is not a valid string; unexpected character `{}`",
                        c
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
                    tokens_found
                        .push((label, token_buf[..token_len].iter().collect()));
                    state = START_STATE;
                    token_len = 0;
                }
                _ => {
                    token_buf[token_len] = c;
                    token_len += 1;
                }
            }
        }
        return Ok(tokens_found);
    }
}
