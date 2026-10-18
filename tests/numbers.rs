use yarlg::{
    AsciiCharacterRange as range, CharacterGroupType::*, Lexer, Token, group,
    table_init,
};

// NUMBER = {0...9}+
// FLOAT  = {0...9}+,{.},{0...9}+
// Both share the `{0...9}+` prefix, the '.' decides which token it is.
const DIGIT: range = range::new().with_start(b'0').with_end(b'9').build();
const DOT: range = range::new().with_start(b'.').build();
const SPACE: range = range::new().with_start(b' ').build();
const NULL: range = range::new().with_start(b'\0').build();

const DELIMITERS: &[yarlg::CharacterGroup] =
    &[group!(SPACE, ExactlyOne), group!(NULL, ExactlyOne)];

const TOKENS: &[Token] = &[
    Token::new("NUMBER", &[group!(DIGIT, AtLeastOne)], DELIMITERS),
    Token::new(
        "FLOAT",
        &[
            group!(DIGIT, AtLeastOne),
            group!(DOT, ExactlyOne),
            group!(DIGIT, AtLeastOne),
        ],
        DELIMITERS,
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
fn numbers_and_floats() {
    assert_eq!(
        lex("1 42 3.14 0.5"),
        Ok(vec![
            tok("NUMBER", "1"),
            tok("NUMBER", "42"),
            tok("FLOAT", "3.14"),
            tok("FLOAT", "0.5"),
        ])
    );
}

#[test]
fn skips_leading_whitespace() {
    assert_eq!(
        lex(" \t\n\r7   8"),
        Ok(vec![tok("NUMBER", "7"), tok("NUMBER", "8")])
    );
}

#[test]
fn empty_input() {
    assert_eq!(lex(""), Ok(vec![]));
    assert_eq!(lex("   "), Ok(vec![]));
}

#[test]
fn rejects_incomplete_float() {
    assert!(lex("1.").is_err());
    assert!(lex(".5").is_err());
    assert!(lex("1.2.3").is_err());
}

#[test]
fn rejects_undefined_characters() {
    assert!(lex("12a").is_err());
    assert!(lex("-1").is_err());
}
