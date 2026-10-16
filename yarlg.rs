// yarlg.rs: Yet another Rust lexer generator.

use core::num;

// Ordinal continguos range of ASCII characters.
// '0'...'9', 'a'...'z'
struct AsciiCharacterRange {
    start: u8,
    end: u8,
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

type State = usize;
const TRANSITIONS_PER_STATE: i32 = 128;
const STATE_INAVLID: usize = 0;

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
    return (number_of_states * TRANSITIONS_PER_STATE) as usize;
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
