use yarlg::{
    AsciiCharacterRange, CharacterGroupType, Lexer, Token, group, table_init,
};

// The README calculator lexer:
// NUMBER = {0...9}+
// FLOAT  = {0...9}+,".",{0...9}+
// OP     = "+","-","/","*"
// EQ     = "="
// every token followed by ' ' or end of input.
const DIGIT: AsciiCharacterRange = AsciiCharacterRange::new()
    .with_start(b'0')
    .with_end(b'9')
    .build();
const DOT: AsciiCharacterRange =
    AsciiCharacterRange::new().with_start(b'.').build();
const PLUS: AsciiCharacterRange =
    AsciiCharacterRange::new().with_start(b'+').build();
const MINUS: AsciiCharacterRange =
    AsciiCharacterRange::new().with_start(b'-').build();
const SLASH: AsciiCharacterRange =
    AsciiCharacterRange::new().with_start(b'/').build();
const STAR: AsciiCharacterRange =
    AsciiCharacterRange::new().with_start(b'*').build();
const EQUAL: AsciiCharacterRange =
    AsciiCharacterRange::new().with_start(b'=').build();
const SPACE: AsciiCharacterRange =
    AsciiCharacterRange::new().with_start(b' ').build();
const NULL: AsciiCharacterRange =
    AsciiCharacterRange::new().with_start(b'\0').build();

const TOKENS: &[Token] = &[
    Token::new(
        "NUMBER",
        &[group!(DIGIT, CharacterGroupType::AtLeastOne)],
        &[
            group!(SPACE, CharacterGroupType::ExactlyOne),
            group!(NULL, CharacterGroupType::ExactlyOne),
        ],
    ),
    Token::new(
        "FLOAT",
        &[
            group!(DIGIT, CharacterGroupType::AtLeastOne),
            group!(DOT, CharacterGroupType::ExactlyOne),
            group!(DIGIT, CharacterGroupType::AtLeastOne),
        ],
        &[
            group!(SPACE, CharacterGroupType::ExactlyOne),
            group!(NULL, CharacterGroupType::ExactlyOne),
        ],
    ),
    Token::new(
        "OP",
        &[group!(PLUS MINUS SLASH STAR, CharacterGroupType::ExactlyOne)],
        &[
            group!(SPACE, CharacterGroupType::ExactlyOne),
            group!(NULL, CharacterGroupType::ExactlyOne),
        ],
    ),
    Token::new(
        "EQ",
        &[group!(EQUAL, CharacterGroupType::ExactlyOne)],
        &[
            group!(SPACE, CharacterGroupType::ExactlyOne),
            group!(NULL, CharacterGroupType::ExactlyOne),
        ],
    ),
];

table_init!(LEXER, TOKENS);

fn lex(input: &str) -> Result<Vec<(&'static str, String)>, String> {
    Lexer::run(input, &LEXER)
}

fn tok(label: &'static str, text: &str) -> (&'static str, String) {
    (label, text.to_string())
}

#[test]
fn lexes_expression() {
    assert_eq!(
        lex("12 + 3.5 = 15.5").unwrap(),
        vec![
            tok("NUMBER", "12"),
            tok("OP", "+"),
            tok("FLOAT", "3.5"),
            tok("EQ", "="),
            tok("FLOAT", "15.5"),
        ]
    );
}

#[test]
fn lexes_every_operator() {
    assert_eq!(
        lex("1 + 2 - 3 / 4 * 5").unwrap(),
        vec![
            tok("NUMBER", "1"),
            tok("OP", "+"),
            tok("NUMBER", "2"),
            tok("OP", "-"),
            tok("NUMBER", "3"),
            tok("OP", "/"),
            tok("NUMBER", "4"),
            tok("OP", "*"),
            tok("NUMBER", "5"),
        ]
    );
}

#[test]
fn number_alone() {
    assert_eq!(lex("42").unwrap(), vec![tok("NUMBER", "42")]);
}

#[test]
fn float_alone() {
    assert_eq!(lex("3.14").unwrap(), vec![tok("FLOAT", "3.14")]);
}

#[test]
fn multi_digit_floats() {
    // NUMBER and FLOAT share the `{0...9}+` prefix, so every digit must
    // keep looping on the merged state, not only the last one written ('9').
    for s in ["10.5", "15.5", "123.4", "99.5"] {
        assert_eq!(lex(s).unwrap(), vec![tok("FLOAT", s)], "input {s:?}");
    }
}

#[test]
fn number_after_shared_prefix() {
    assert_eq!(
        lex("15 3").unwrap(),
        vec![tok("NUMBER", "15"), tok("NUMBER", "3")]
    );
}

#[test]
fn float_then_number() {
    assert_eq!(
        lex("1.5 7").unwrap(),
        vec![tok("FLOAT", "1.5"), tok("NUMBER", "7")]
    );
}

#[test]
fn input_ending_with_delimiter() {
    assert_eq!(
        lex("1 = ").unwrap(),
        vec![tok("NUMBER", "1"), tok("EQ", "=")]
    );
}

#[test]
fn empty_input() {
    assert_eq!(lex("").unwrap(), vec![]);
}

#[test]
fn rejects_incomplete_float() {
    assert!(lex("1.").is_err());
    assert!(lex("1. ").is_err());
}

#[test]
fn rejects_float_without_integer_part() {
    assert!(lex(".5").is_err());
}

#[test]
fn rejects_two_dots() {
    assert!(lex("1.2.3").is_err());
}

#[test]
fn rejects_consecutive_operators() {
    assert!(lex("++").is_err());
}

#[test]
fn rejects_unknown_character() {
    assert!(lex("1 % 2").is_err());
}

#[test]
fn rejects_operators_without_spaces() {
    // Delimiters are consumed, so tokens must be separated by ' '.
    assert!(lex("1+2").is_err());
}
