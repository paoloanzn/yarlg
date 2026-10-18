use yarlg::{
    AsciiCharacterRange, CharacterGroupType, Lexer, Token, group, table_init,
};

// A programming-language style lexer:
// IDENT  = {a...z, A...Z, _},{a...z, A...Z, 0...9, _}*
// NUMBER = {0...9}+
// every token followed by ' ' or end of input.
const LOWER: AsciiCharacterRange = AsciiCharacterRange::new()
    .with_start(b'a')
    .with_end(b'z')
    .build();
const UPPER: AsciiCharacterRange = AsciiCharacterRange::new()
    .with_start(b'A')
    .with_end(b'Z')
    .build();
const DIGIT: AsciiCharacterRange = AsciiCharacterRange::new()
    .with_start(b'0')
    .with_end(b'9')
    .build();
const UNDERSCORE: AsciiCharacterRange =
    AsciiCharacterRange::new().with_start(b'_').build();
const SPACE: AsciiCharacterRange =
    AsciiCharacterRange::new().with_start(b' ').build();
const NULL: AsciiCharacterRange =
    AsciiCharacterRange::new().with_start(b'\0').build();

const TOKENS: &[Token] = &[
    Token::new(
        "IDENT",
        &[
            group!(LOWER UPPER UNDERSCORE, CharacterGroupType::ExactlyOne),
            group!(
                LOWER UPPER DIGIT UNDERSCORE,
                CharacterGroupType::ZeroOrMore
            ),
        ],
        &[
            group!(SPACE, CharacterGroupType::ExactlyOne),
            group!(NULL, CharacterGroupType::ExactlyOne),
        ],
    ),
    Token::new(
        "NUMBER",
        &[group!(DIGIT, CharacterGroupType::AtLeastOne)],
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
fn lexes_idents_and_numbers() {
    assert_eq!(
        lex("foo 42 Bar_1 7").unwrap(),
        vec![
            tok("IDENT", "foo"),
            tok("NUMBER", "42"),
            tok("IDENT", "Bar_1"),
            tok("NUMBER", "7"),
        ]
    );
}

#[test]
fn single_character_ident() {
    // The ZeroOrMore tail may match nothing.
    for s in ["x", "Q", "_"] {
        assert_eq!(lex(s).unwrap(), vec![tok("IDENT", s)], "input {s:?}");
    }
}

#[test]
fn ident_tail_accepts_digits_and_underscores() {
    for s in ["x1", "snake_case", "CamelCase", "__init__", "a1b2c3"] {
        assert_eq!(lex(s).unwrap(), vec![tok("IDENT", s)], "input {s:?}");
    }
}

#[test]
fn number_alone() {
    assert_eq!(lex("1337").unwrap(), vec![tok("NUMBER", "1337")]);
}

#[test]
fn input_ending_with_delimiter() {
    assert_eq!(lex("foo ").unwrap(), vec![tok("IDENT", "foo")]);
}

#[test]
fn empty_input() {
    assert_eq!(lex("").unwrap(), vec![]);
}

#[test]
fn rejects_ident_starting_with_digit() {
    assert!(lex("9lives").is_err());
}

#[test]
fn rejects_double_space() {
    // A delimiter is ExactlyOne, so a second ' ' starts no token.
    assert!(lex("foo  bar").is_err());
}

#[test]
fn rejects_leading_space() {
    assert!(lex(" foo").is_err());
}

#[test]
fn rejects_unknown_character() {
    assert!(lex("foo-bar").is_err());
}

#[test]
fn group_display() {
    let tail =
        group!(LOWER UPPER DIGIT UNDERSCORE, CharacterGroupType::ZeroOrMore);
    assert_eq!(tail.to_string(), "{'a'...'z', 'A'...'Z', '0'...'9', '_'}*");
    let number = group!(DIGIT, CharacterGroupType::AtLeastOne);
    assert_eq!(number.to_string(), "{'0'...'9'}+");
}
