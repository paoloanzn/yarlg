use yarlg::{
    AsciiCharacterRange as range, CharacterGroupType::*, Lexer, Token, group,
    table_init,
};

// ASSIGN = {=}
// EQ     = {=},{=}
// MINUS  = {-}
// ARROW  = {-},{>}
// Each two-char operator starts with a one-char operator.
const EQUAL: range = range::new().with_start(b'=').build();
const DASH: range = range::new().with_start(b'-').build();
const GREATER: range = range::new().with_start(b'>').build();
const SPACE: range = range::new().with_start(b' ').build();
const NULL: range = range::new().with_start(b'\0').build();

const DELIMITERS: &[yarlg::CharacterGroup] =
    &[group!(SPACE, ExactlyOne), group!(NULL, ExactlyOne)];

const TOKENS: &[Token] = &[
    Token::new("ASSIGN", &[group!(EQUAL, ExactlyOne)], DELIMITERS),
    Token::new(
        "EQ",
        &[group!(EQUAL, ExactlyOne), group!(EQUAL, ExactlyOne)],
        DELIMITERS,
    ),
    Token::new("MINUS", &[group!(DASH, ExactlyOne)], DELIMITERS),
    Token::new(
        "ARROW",
        &[group!(DASH, ExactlyOne), group!(GREATER, ExactlyOne)],
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
fn operators() {
    assert_eq!(
        lex("= == - ->"),
        Ok(vec![
            tok("ASSIGN", "="),
            tok("EQ", "=="),
            tok("MINUS", "-"),
            tok("ARROW", "->"),
        ])
    );
}

#[test]
fn same_operator_repeated() {
    assert_eq!(
        lex("-> -> =="),
        Ok(vec![
            tok("ARROW", "->"),
            tok("ARROW", "->"),
            tok("EQ", "==")
        ])
    );
}

#[test]
fn rejects_operators_without_delimiter() {
    assert!(lex("===").is_err());
    assert!(lex("=-").is_err());
    assert!(lex("->>").is_err());
}

#[test]
fn rejects_undefined_characters() {
    assert!(lex(">").is_err());
    assert!(lex("+").is_err());
}
