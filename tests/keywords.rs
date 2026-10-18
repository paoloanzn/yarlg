use yarlg::{
    AsciiCharacterRange, CharacterGroupType, Lexer, Token, group, table_init,
};

// Keyword vs identifier overlap:
// WORD = {a...z}+
// IF   = {i},{f}
const LOWER: AsciiCharacterRange = AsciiCharacterRange::new()
    .with_start(b'a')
    .with_end(b'z')
    .build();
const I: AsciiCharacterRange =
    AsciiCharacterRange::new().with_start(b'i').build();
const F: AsciiCharacterRange =
    AsciiCharacterRange::new().with_start(b'f').build();
const SPACE: AsciiCharacterRange =
    AsciiCharacterRange::new().with_start(b' ').build();
const NULL: AsciiCharacterRange =
    AsciiCharacterRange::new().with_start(b'\0').build();

const DELIMITERS: &[yarlg::CharacterGroup] = &[
    group!(SPACE, CharacterGroupType::ExactlyOne),
    group!(NULL, CharacterGroupType::ExactlyOne),
];

fn tok(label: &'static str, text: &str) -> (&'static str, String) {
    (label, text.to_string())
}

const WORD_FIRST: &[Token] = &[
    Token::new(
        "WORD",
        &[group!(LOWER, CharacterGroupType::AtLeastOne)],
        DELIMITERS,
    ),
    Token::new(
        "IF",
        &[
            group!(I, CharacterGroupType::ExactlyOne),
            group!(F, CharacterGroupType::ExactlyOne),
        ],
        DELIMITERS,
    ),
];

table_init!(LEXER_WORD_FIRST, WORD_FIRST);

#[test]
fn word_declared_first() {
    assert_eq!(
        Lexer::run("if iff i is", &LEXER_WORD_FIRST).unwrap(),
        vec![
            tok("IF", "if"),
            tok("WORD", "iff"),
            tok("WORD", "i"),
            tok("WORD", "is"),
        ]
    );
}
