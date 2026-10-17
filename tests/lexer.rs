use yarlg::{AsciiCharacterRange, CharacterGroupType, Lexer, Token, group, table_init};

// A simple testing lexer:
// NUMBER {'0'...'9'}+ ' '
// IDENT  {'a'...'z'}+ ' '
const DIGIT: AsciiCharacterRange =
    AsciiCharacterRange::new().with_start(b'0').with_end(b'9').build();
const LOWER: AsciiCharacterRange =
    AsciiCharacterRange::new().with_start(b'a').with_end(b'z').build();
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
        "IDENT",
        &[group!(LOWER, CharacterGroupType::AtLeastOne)],
        &[
            group!(SPACE, CharacterGroupType::ExactlyOne),
            group!(NULL, CharacterGroupType::ExactlyOne),
        ],
    ),
];

table_init!(LEXER, TOKENS);

#[test]
fn lexes_numbers_and_idents() {
    let tokens = Lexer::run("42 foo 1337 bar 7", &LEXER).unwrap();
    assert_eq!(
        tokens,
        vec![
            ("NUMBER", "42".to_string()),
            ("IDENT", "foo".to_string()),
            ("NUMBER", "1337".to_string()),
            ("IDENT", "bar".to_string()),
            ("NUMBER", "7".to_string()),
        ]
    );
}

#[test]
fn input_ending_with_delimiter() {
    let tokens = Lexer::run("42 ", &LEXER).unwrap();
    assert_eq!(tokens, vec![("NUMBER", "42".to_string())]);
}

#[test]
fn empty_input() {
    assert_eq!(Lexer::run("", &LEXER).unwrap(), vec![]);
}

#[test]
fn rejects_unexpected_character() {
    assert!(Lexer::run("42 Foo ", &LEXER).is_err());
}

#[test]
fn group_display() {
    let group = group!(DIGIT, CharacterGroupType::AtLeastOne);
    assert_eq!(group.to_string(), "{'0'...'9'}+");
}
