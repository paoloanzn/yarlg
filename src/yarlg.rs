//! yarlg.rs: Yet another Rust lexer generator.
//!
//! `yarlg` builds the transition table of a lexer DFA entirely at compile
//! time from a `const` description of your tokens. At runtime, lexing is a
//! single table lookup per input character.
//!
//! # Overview
//!
//! A lexer is described bottom-up:
//!
//! 1. [`AsciiCharacterRange`]: a contiguous range of ASCII characters
//!    (`'0'...'9'`) or a single character (`' '`).
//! 2. [`CharacterGroup`]: a set of ranges plus a repetition
//!    ([`CharacterGroupType`]), usually created with the [`group!`] macro.
//!    `{'a'...'z', 'A'...'Z'}+` is one group.
//! 3. [`Token`]: a label, a sequence of groups that the token must match in
//!    order, and the delimiter groups that end it.
//! 4. [`table_init!`]: turns a `const` slice of tokens into a `const`
//!    [`TransitionTable`].
//! 5. [`Lexer::run`]: lexes a string with that table.
//!
//! # Example
//!
//! ```
//! use yarlg::{AsciiCharacterRange, CharacterGroupType, Token, group, table_init};
//!
//! const DIGIT: AsciiCharacterRange =
//!     AsciiCharacterRange::new().with_start(b'0').with_end(b'9').build();
//! const LOWER: AsciiCharacterRange =
//!     AsciiCharacterRange::new().with_start(b'a').with_end(b'z').build();
//! const SPACE: AsciiCharacterRange =
//!     AsciiCharacterRange::new().with_start(b' ').build();
//! // End of input is signalled with '\0', see `Lexer::run`.
//! const NULL: AsciiCharacterRange =
//!     AsciiCharacterRange::new().with_start(b'\0').build();
//!
//! // NUMBER {'0'...'9'}+ followed by ' ' or end of input
//! // IDENT  {'a'...'z'}+ followed by ' ' or end of input
//! const TOKENS: &[Token] = &[
//!     Token::new(
//!         "NUMBER",
//!         &[group!(DIGIT, CharacterGroupType::AtLeastOne)],
//!         &[
//!             group!(SPACE, CharacterGroupType::ExactlyOne),
//!             group!(NULL, CharacterGroupType::ExactlyOne),
//!         ],
//!     ),
//!     Token::new(
//!         "IDENT",
//!         &[group!(LOWER, CharacterGroupType::AtLeastOne)],
//!         &[
//!             group!(SPACE, CharacterGroupType::ExactlyOne),
//!             group!(NULL, CharacterGroupType::ExactlyOne),
//!         ],
//!     ),
//! ];
//!
//! table_init!(LEXER, TOKENS);
//!
//! let tokens = yarlg::Lexer::run("42 foo 7", &LEXER).unwrap();
//! assert_eq!(
//!     tokens,
//!     vec![
//!         ("NUMBER", String::from("42")),
//!         ("IDENT", String::from("foo")),
//!         ("NUMBER", String::from("7")),
//!     ]
//! );
//!
//! // Uppercase letters are not part of any token.
//! assert!(yarlg::Lexer::run("42 Foo", &LEXER).is_err());
//! ```
//!
//! # Limitations
//!
//! - Only ASCII input (`0..=127`) is supported.
//! - A single token can be at most 4096 characters long.

use std::fmt::{self, Debug};

/// A contiguous range of ASCII characters, or a single ASCII character.
///
/// A range is built with [`new`](Self::new), optionally given a
/// [`start`](Self::with_start) and an [`end`](Self::with_end), and validated
/// with [`build`](Self::build). Depending on which bounds are set it matches:
///
/// | `start` | `end`  | matches                          |
/// |---------|--------|----------------------------------|
/// | `Some`  | `Some` | every character in `start..=end` |
/// | `Some`  | `None` | only `start`                     |
/// | `None`  | `Some` | only `end`                       |
/// | `None`  | `None` | nothing                          |
///
/// All constructors are `const fn`, so ranges are normally declared as
/// `const` items and validated at compile time.
///
/// # Examples
///
/// ```
/// use yarlg::AsciiCharacterRange;
///
/// // DIGIT '0'...'9'
/// const DIGIT: AsciiCharacterRange =
///     AsciiCharacterRange::new().with_start(b'0').with_end(b'9').build();
/// // SPACE ' '
/// const SPACE: AsciiCharacterRange =
///     AsciiCharacterRange::new().with_start(b' ').build();
///
/// assert_eq!(DIGIT.to_string(), "'0'...'9'");
/// assert_eq!(SPACE.to_string(), "' '");
/// ```
#[derive(Debug, Default)]
pub struct AsciiCharacterRange {
    start: Option<u8>,
    end: Option<u8>,
}

impl AsciiCharacterRange {
    /// Creates an empty range with neither `start` nor `end` set.
    ///
    /// An empty range matches no character. Set its bounds with
    /// [`with_start`](Self::with_start) and [`with_end`](Self::with_end),
    /// then call [`build`](Self::build).
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

    /// Validates the range and returns it.
    ///
    /// Always call `build` last. In a `const` context, an invalid range
    /// fails compilation.
    ///
    /// # Panics
    ///
    /// Panics if `start` or `end` is not ASCII (greater than 127), or if
    /// `start` is greater than `end`.
    ///
    /// ```compile_fail
    /// // 'z' > 'a': rejected at compile time.
    /// const BAD: yarlg::AsciiCharacterRange =
    ///     yarlg::AsciiCharacterRange::new().with_start(b'z').with_end(b'a').build();
    /// ```
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

/// How many characters a [`CharacterGroup`] matches.
pub enum CharacterGroupType {
    /// Exactly one character from the group: `{...}`.
    ExactlyOne,
    /// One or more characters from the group: `{...}+`.
    AtLeastOne,
    /// Zero or more characters from the group: `{...}*`.
    ///
    /// Not allowed as a [`Token`] delimiter.
    ZeroOrMore,
}

/// A set of ASCII character ranges with a repetition, such as
/// `{'0'...'9', 'a'...'z', 'A'...'Z'}+`.
///
/// A character matches the group if it falls in any of its `ranges`;
/// `group_type` says how many such characters are matched. Groups are
/// usually created with the [`group!`] macro and used to describe the body
/// and the delimiters of a [`Token`].
///
/// # Examples
///
/// ```
/// use yarlg::{AsciiCharacterRange, CharacterGroupType, group};
///
/// const LOWER: AsciiCharacterRange =
///     AsciiCharacterRange::new().with_start(b'a').with_end(b'z').build();
/// const UNDERSCORE: AsciiCharacterRange =
///     AsciiCharacterRange::new().with_start(b'_').build();
///
/// let word = group!(LOWER UNDERSCORE, CharacterGroupType::AtLeastOne);
/// assert_eq!(word.to_string(), "{'a'...'z', '_'}+");
/// ```
pub struct CharacterGroup<'a> {
    /// The ranges a character can belong to.
    pub ranges: &'a [AsciiCharacterRange],
    /// How many characters from `ranges` the group matches.
    pub group_type: CharacterGroupType,
}

/// Creates a [`CharacterGroup`] from one or more [`AsciiCharacterRange`]s
/// and a [`CharacterGroupType`].
///
/// Ranges are separated by whitespace; the group type comes last, after a
/// comma:
///
/// ```
/// use yarlg::{AsciiCharacterRange, CharacterGroupType, group};
///
/// const DIGIT: AsciiCharacterRange =
///     AsciiCharacterRange::new().with_start(b'0').with_end(b'9').build();
/// const LOWER: AsciiCharacterRange =
///     AsciiCharacterRange::new().with_start(b'a').with_end(b'z').build();
///
/// let number = group!(DIGIT, CharacterGroupType::AtLeastOne);
/// let alnum = group!(DIGIT LOWER, CharacterGroupType::ZeroOrMore);
/// ```
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

/// A token definition: a label, the groups it is made of and the delimiters
/// that end it.
///
/// Input matches the token when it matches each of `groups` in order and is
/// then followed by a character matched by any of `delimiters`. The
/// delimiter is consumed but not included in the token text returned by
/// [`Lexer::run`].
///
/// Tokens are declared in a `const` slice and passed to [`table_init!`].
/// See the [crate-level example](crate#example).
pub struct Token<'a> {
    label: &'a str,
    groups: &'a [CharacterGroup<'a>],
    // TODO: evaluate if replacing [CharacterGroup] with one single group of ranges, CharacterGroup, is better.
    // The reason is that logically this will always be one single group.
    delimiters: &'a [CharacterGroup<'a>],
}

impl<'a> Token<'a> {
    /// Creates a new token definition.
    ///
    /// - `label`: the name reported by [`Lexer::run`] for this token.
    /// - `groups`: the groups the token body must match, in order.
    /// - `delimiters`: groups that end the token. Any one of them ends it.
    ///
    /// To accept the token at the end of the input, include a delimiter
    /// that matches `'\0'` (see [`Lexer::run`]).
    ///
    /// # Panics
    ///
    /// The definition is checked when the table is built by
    /// [`table_init!`], not here: a delimiter group of type
    /// [`CharacterGroupType::ZeroOrMore`] fails compilation there.
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

/// A DFA state in a [`TransitionTable`].
///
/// `-1` is the invalid state. Other negative values mark accepting
/// (terminal) states.
pub type State = isize;
const INVALID_STATE: isize = -1;
const START_STATE: State = 0;
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
/// Returns the number of entries in a [`TransitionTable`] with
/// `number_of_states` states (128 entries per state).
///
/// Used by [`table_init!`]; you don't normally call it directly.
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

// Terminal states are stored as negative values: a transition into -4 means
// "token accepted"; its outgoing transitions are still at row 4.
/// The compiled lexer DFA.
///
/// Create it with [`table_init!`] and pass it to [`Lexer::run`].
///
/// - `N` is the table size: number of states × 128 ASCII characters.
/// - `M` is the number of tokens.
pub struct TransitionTable<'a, const N: usize, const M: usize> {
    /// Flat transition table: the next state for `(state, c)` is at
    /// `state * 128 + c`. `-1` means no transition.
    pub table: [State; N],
    /// The accepting state of each token, paired with the token label.
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

/// Returns the total number of states of the DFA for `tokens`.
///
/// The start state is shared by every token. Used by [`table_init!`]; you
/// don't normally call it directly.
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

/// Returns the largest number of transitions generated by a single token.
///
/// Sizes the scratch buffer of [`build_table`]. Used by [`table_init!`];
/// you don't normally call it directly.
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

/// Defines a `const` [`TransitionTable`] named `$name` from `$tokens`.
///
/// `$tokens` must be a `const` of type `&[Token]`. The table is computed
/// entirely at compile time, and its sizes are derived from the token
/// definitions, so you never write the generic parameters yourself.
///
/// ```ignore
/// table_init!(LEXER, TOKENS);
/// let tokens = yarlg::Lexer::run("42 foo", &LEXER);
/// ```
///
/// See the [crate-level example](crate#example) for a complete lexer.
///
/// # Panics
///
/// Since the table is a `const`, an invalid token definition fails
/// compilation; for example, a delimiter group of type
/// [`CharacterGroupType::ZeroOrMore`].
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
/// `N` is the table size, `M` the number of tokens and `T` the size of the
/// scratch transition buffer. Use [`table_init!`] instead of calling this
/// directly: it computes all three for you.
///
/// # Panics
///
/// Panics if `M` differs from `tokens.len()` or if a token has a
/// [`CharacterGroupType::ZeroOrMore`] delimiter.
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

/// Runs a lexer built with [`table_init!`] on input strings.
pub struct Lexer {}

const TOKEN_MAX_LEN: usize = 4096;

impl Lexer {
    /// Splits `input` into tokens using `table`.
    ///
    /// Returns the tokens in input order as `(label, text)` pairs, where
    /// `label` is the [`Token`] label and `text` is the matched text
    /// without its delimiter.
    ///
    /// The end of the input is signalled by a trailing `'\0'`. If the input
    /// does not already end with a delimiter, the last token is accepted only
    /// if one of its delimiters matches `'\0'`.
    ///
    /// # Errors
    ///
    /// Returns an error message if `input` contains a character that no
    /// token accepts at that position, or if it ends in the middle of a
    /// token that has no `'\0'` delimiter.
    ///
    /// # Panics
    ///
    /// Panics if a single token is longer than 4096 characters.
    ///
    /// # Examples
    ///
    /// ```
    /// use yarlg::{AsciiCharacterRange, CharacterGroupType, Token, group, table_init};
    ///
    /// const DIGIT: AsciiCharacterRange =
    ///     AsciiCharacterRange::new().with_start(b'0').with_end(b'9').build();
    /// const COMMA: AsciiCharacterRange =
    ///     AsciiCharacterRange::new().with_start(b',').build();
    ///
    /// // NUMBER {'0'...'9'}+ ','  (no '\0' delimiter)
    /// const TOKENS: &[Token] = &[Token::new(
    ///     "NUMBER",
    ///     &[group!(DIGIT, CharacterGroupType::AtLeastOne)],
    ///     &[group!(COMMA, CharacterGroupType::ExactlyOne)],
    /// )];
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
        // A trailing '\0' terminates the last token, so it must be a delimiter
        // of every token.
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
