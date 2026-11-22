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

// NUMBER = {0...9}+  peeking {+, -, ' ', \0}
// PLUS   = {+}       peeking {0...9, ' ', \0}
// MINUS  = {-}       peeking {0...9, ' ', \0}
// Peeked delimiters are lexed again, so no whitespace is needed between tokens.
#[test]
fn adjacent_tokens_with_peeked_delimiters() {
    const DIGIT: range = range::new().with_start('0').with_end('9').build();
    const PLUS: range = range::new().with_start('+').build();
    const RULES: &[Token] = &[
        Token::new(
            "NUMBER",
            &[group!(DIGIT, AtLeastOne)],
            &[group!(PLUS DASH SPACE NULL, ExactlyOne)],
        )
        .peek_delimiters(),
        Token::new(
            "PLUS",
            &[group!(PLUS, ExactlyOne)],
            &[group!(DIGIT SPACE NULL, ExactlyOne)],
        )
        .peek_delimiters(),
        Token::new(
            "MINUS",
            &[group!(DASH, ExactlyOne)],
            &[group!(DIGIT SPACE NULL, ExactlyOne)],
        )
        .peek_delimiters(),
    ];
    table_init!(ARITHMETIC, RULES);
    let expected = Ok(vec![
        tok("NUMBER", "12"),
        tok("PLUS", "+"),
        tok("NUMBER", "34"),
        tok("MINUS", "-"),
        tok("NUMBER", "5"),
    ]);
    assert_eq!(Lexer::run("12+34-5", &ARITHMETIC), expected);
    // Peeked whitespace is skipped, so it behaves like a consumed one.
    assert_eq!(Lexer::run(" 12 + 34 - 5 ", &ARITHMETIC), expected);
    // `--` is not a token: MINUS doesn't accept '-' after itself.
    assert!(Lexer::run("1--2", &ARITHMETIC).is_err());
}

#[test]
fn peeked_delimiter_must_start_a_token() {
    const DIGIT: range = range::new().with_start('0').with_end('9').build();
    const PLUS: range = range::new().with_start('+').build();
    const RULES: &[Token] = &[Token::new(
        "NUMBER",
        &[group!(DIGIT, AtLeastOne)],
        &[group!(PLUS NULL, ExactlyOne)],
    )
    .peek_delimiters()];
    table_init!(NUMBERS, RULES);
    assert!(Lexer::run("1+2", &NUMBERS).unwrap_err().contains('+'));
}

// WORD   = {a...z}+  consuming {\n, \0}
// WORD   = {a...z}+  peeking {,}
// COMMA  = {,}       peeking {a...z}
// Same pattern and label, different delimiters: '\n' is dropped, ',' is kept.
#[test]
fn consumed_and_peeked_delimiters_with_the_same_label() {
    const LOWER: range = range::new().with_start('a').with_end('z').build();
    const COMMA: range = range::new().with_start(',').build();
    const NEWLINE: range = range::new().with_start('\n').build();
    const RULES: &[Token] = &[
        Token::new(
            "WORD",
            &[group!(LOWER, AtLeastOne)],
            &[group!(NEWLINE NULL, ExactlyOne)],
        ),
        Token::new(
            "WORD",
            &[group!(LOWER, AtLeastOne)],
            &[group!(COMMA, ExactlyOne)],
        )
        .peek_delimiters(),
        Token::new(
            "COMMA",
            &[group!(COMMA, ExactlyOne)],
            &[group!(LOWER, ExactlyOne)],
        )
        .peek_delimiters(),
    ];
    table_init!(LIST, RULES);
    assert_eq!(
        Lexer::run("a,bc\nd,e", &LIST),
        Ok(vec![
            tok("WORD", "a"),
            tok("COMMA", ","),
            tok("WORD", "bc"),
            tok("WORD", "d"),
            tok("COMMA", ","),
            tok("WORD", "e"),
        ])
    );
}

// The delimiter is lexed again from its first byte, not from the one
// that triggered the transition.
#[test]
fn multibyte_peeked_delimiter() {
    const DIGIT: range = range::new().with_start('0').with_end('9').build();
    const ARROW: range = range::new().with_start('→').build();
    const RULES: &[Token] = &[
        Token::new(
            "NUMBER",
            &[group!(DIGIT, AtLeastOne)],
            &[group!(ARROW NULL, ExactlyOne)],
        )
        .peek_delimiters(),
        Token::new(
            "ARROW",
            &[group!(ARROW, ExactlyOne)],
            &[group!(DIGIT, ExactlyOne)],
        )
        .peek_delimiters(),
    ];
    table_init!(ARROWS, RULES);
    assert_eq!(
        Lexer::run("1→22→3", &ARROWS),
        Ok(vec![
            tok("NUMBER", "1"),
            tok("ARROW", "→"),
            tok("NUMBER", "22"),
            tok("ARROW", "→"),
            tok("NUMBER", "3"),
        ])
    );
}

// A token that can be empty must not peek its delimiter forever.
#[test]
fn empty_token_consumes_its_peeked_delimiter() {
    const DIGIT: range = range::new().with_start('0').with_end('9').build();
    const PLUS: range = range::new().with_start('+').build();
    const RULES: &[Token] = &[Token::new(
        "DIGITS",
        &[group!(DIGIT, ZeroOrMore)],
        &[group!(PLUS NULL, ExactlyOne)],
    )
    .peek_delimiters()];
    table_init!(DIGITS, RULES);
    assert_eq!(
        Lexer::run("1+2", &DIGITS),
        Ok(vec![
            tok("DIGITS", "1"),
            tok("DIGITS", ""),
            tok("DIGITS", "2")
        ])
    );
}
