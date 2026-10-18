use yarlg::{
    AsciiCharacterRange as range, CharacterGroupType::*, Lexer, Token, group,
    table_init,
};

// KEY   = {a...z, 0...9, _}+ delimited by '='
// VALUE = {a...z, 0...9, _}+ delimited by '\n' or end of input
// Both tokens have the same groups: only the delimiter decides the label.
const LOWER: range = range::new().with_start(b'a').with_end(b'z').build();
const DIGIT: range = range::new().with_start(b'0').with_end(b'9').build();
const UNDERSCORE: range = range::new().with_start(b'_').build();
const EQUAL: range = range::new().with_start(b'=').build();
const NEWLINE: range = range::new().with_start(b'\n').build();
const NULL: range = range::new().with_start(b'\0').build();

const TOKENS: &[Token] = &[
    Token::new(
        "KEY",
        &[group!(LOWER DIGIT UNDERSCORE, AtLeastOne)],
        &[group!(EQUAL, ExactlyOne)],
    ),
    Token::new(
        "VALUE",
        &[group!(LOWER DIGIT UNDERSCORE, AtLeastOne)],
        &[group!(NEWLINE, ExactlyOne), group!(NULL, ExactlyOne)],
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
fn key_value_pairs() {
    assert_eq!(
        lex("host=localhost\nport=8080\nlog_level=debug"),
        Ok(vec![
            tok("KEY", "host"),
            tok("VALUE", "localhost"),
            tok("KEY", "port"),
            tok("VALUE", "8080"),
            tok("KEY", "log_level"),
            tok("VALUE", "debug"),
        ])
    );
}

#[test]
fn delimiter_decides_label() {
    assert_eq!(lex("abc="), Ok(vec![tok("KEY", "abc")]));
    assert_eq!(lex("abc\n"), Ok(vec![tok("VALUE", "abc")]));
    assert_eq!(lex("abc"), Ok(vec![tok("VALUE", "abc")]));
    assert_eq!(lex("a=b="), Ok(vec![tok("KEY", "a"), tok("KEY", "b")]));
}

#[test]
fn skips_blank_lines() {
    assert_eq!(
        lex("\n\na=1\n\n\nb=2\n"),
        Ok(vec![
            tok("KEY", "a"),
            tok("VALUE", "1"),
            tok("KEY", "b"),
            tok("VALUE", "2"),
        ])
    );
}

#[test]
fn rejects_undefined_characters() {
    assert!(lex("=localhost").is_err());
    assert!(lex("Host=localhost").is_err());
    assert!(lex("host=local host").is_err());
}
