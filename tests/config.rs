use yarlg::{
    AsciiCharacterRange, CharacterGroupType, Lexer, Token, group, table_init,
};

// A `key=value` config file lexer:
// KEY   = {a...z, 0...9, _}+  followed by '='
// VALUE = {a...z, 0...9, _}+  followed by '\n' or end of input.
// Both tokens share the same body: only the delimiter decides the label.
const LOWER: AsciiCharacterRange = AsciiCharacterRange::new()
    .with_start(b'a')
    .with_end(b'z')
    .build();
const DIGIT: AsciiCharacterRange = AsciiCharacterRange::new()
    .with_start(b'0')
    .with_end(b'9')
    .build();
const UNDERSCORE: AsciiCharacterRange =
    AsciiCharacterRange::new().with_start(b'_').build();
const EQUAL: AsciiCharacterRange =
    AsciiCharacterRange::new().with_start(b'=').build();
const NEWLINE: AsciiCharacterRange =
    AsciiCharacterRange::new().with_start(b'\n').build();
const NULL: AsciiCharacterRange =
    AsciiCharacterRange::new().with_start(b'\0').build();

const TOKENS: &[Token] = &[
    Token::new(
        "KEY",
        &[group!(LOWER DIGIT UNDERSCORE, CharacterGroupType::AtLeastOne)],
        &[group!(EQUAL, CharacterGroupType::ExactlyOne)],
    ),
    Token::new(
        "VALUE",
        &[group!(LOWER DIGIT UNDERSCORE, CharacterGroupType::AtLeastOne)],
        &[
            group!(NEWLINE, CharacterGroupType::ExactlyOne),
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
fn lexes_config_file() {
    assert_eq!(
        lex("host=localhost\nport=8080\nlog_level=debug").unwrap(),
        vec![
            tok("KEY", "host"),
            tok("VALUE", "localhost"),
            tok("KEY", "port"),
            tok("VALUE", "8080"),
            tok("KEY", "log_level"),
            tok("VALUE", "debug"),
        ]
    );
}

#[test]
fn delimiter_decides_label() {
    assert_eq!(lex("abc=").unwrap(), vec![tok("KEY", "abc")]);
    assert_eq!(lex("abc\n").unwrap(), vec![tok("VALUE", "abc")]);
    assert_eq!(lex("abc").unwrap(), vec![tok("VALUE", "abc")]);
}

#[test]
fn trailing_newline() {
    assert_eq!(
        lex("debug=true\n").unwrap(),
        vec![tok("KEY", "debug"), tok("VALUE", "true")]
    );
}

#[test]
fn delimiters_are_not_part_of_tokens() {
    for (_, text) in lex("a=1\nb=2\n").unwrap() {
        assert!(!text.contains('=') && !text.contains('\n'), "{text:?}");
    }
}

#[test]
fn consecutive_keys() {
    // The lexer only labels tokens, KEY/VALUE alternation is a parser concern.
    assert_eq!(lex("a=b=").unwrap(), vec![tok("KEY", "a"), tok("KEY", "b")]);
}

#[test]
fn empty_input() {
    assert_eq!(lex("").unwrap(), vec![]);
}

#[test]
fn rejects_empty_value() {
    assert!(lex("host=\n").is_err());
}

#[test]
fn rejects_empty_key() {
    assert!(lex("=localhost").is_err());
}

#[test]
fn rejects_blank_line() {
    assert!(lex("a=1\n\nb=2").is_err());
}

#[test]
fn rejects_spaces_around_equal() {
    assert!(lex("host = localhost").is_err());
}

#[test]
fn rejects_uppercase() {
    assert!(lex("Host=localhost").is_err());
}
