use yarlg::{
    CharacterGroupType::*, CharacterRange as range, Lexer, Token, group,
    table_init,
};

// ASSIGN = {=}
// EQ     = {=},{=}
// MINUS  = {-}
// ARROW  = {-},{>}
// Each two-char operator starts with a one-char operator.
const EQUAL: range = range::new().with_start('=').build();
const DASH: range = range::new().with_start('-').build();
const GREATER: range = range::new().with_start('>').build();
const SPACE: range = range::new().with_start(' ').build();
const NULL: range = range::new().with_start('\0').build();

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

#[test]
fn multibyte_repetition_and_delimiters() {
    const FACE: range = range::new().with_start('😀').with_end('😇').build();
    const END: range = range::new().with_start('終').build();
    const SEP: range = range::new().with_start('🛑').build();
    const RULES: &[Token] = &[Token::new(
        "END",
        &[group!(FACE, ZeroOrMore), group!(END, ExactlyOne)],
        &[group!(SEP NULL, ExactlyOne)],
    )];
    table_init!(UNICODE, RULES);
    assert_eq!(
        Lexer::run("終🛑😀😇終", &UNICODE),
        Ok(vec![tok("END", "終"), tok("END", "😀😇終")])
    );
    assert!(Lexer::run("😀", &UNICODE).is_err());
    assert!(Lexer::run("😈終", &UNICODE).is_err());
}

#[test]
fn broad_scalar_range_fits_a_small_byte_dfa() {
    // Cross all UTF-8 width boundaries and the surrogate gap, with partial
    // byte blocks at both ends. The state budget rules out scalar expansion.
    const BROAD: range =
        range::new().with_start('~').with_end('\u{10abcd}').build();
    const RULES: &[Token] = &[Token::new(
        "SCALAR",
        &[group!(BROAD, ExactlyOne)],
        &[group!(NULL, ExactlyOne)],
    )];
    table_init!(UNICODE, RULES, max_states = 64);
    for c in [
        '~',
        '\u{7f}',
        '\u{80}',
        '\u{7ff}',
        '\u{800}',
        '\u{d7ff}',
        '\u{e000}',
        '\u{ffff}',
        '\u{10000}',
        '\u{10abcd}',
    ] {
        let text = c.to_string();
        assert_eq!(Lexer::run(&text, &UNICODE), Ok(vec![tok("SCALAR", &text)]));
    }
    for c in ['}', '\u{10abce}', char::MAX] {
        assert!(Lexer::run(&c.to_string(), &UNICODE).is_err());
    }
}
