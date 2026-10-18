use yarlg::{
    AsciiCharacterRange as range, CharacterGroupType::*, Lexer, Token, group,
    table_init,
};

// WORD = {a...z}+
// IF   = {i},{f}
// "if" matches both tokens: the last declared one wins.
const LOWER: range = range::new().with_start(b'a').with_end(b'z').build();
const I: range = range::new().with_start(b'i').build();
const F: range = range::new().with_start(b'f').build();
const SPACE: range = range::new().with_start(b' ').build();
const NULL: range = range::new().with_start(b'\0').build();

const DELIMITERS: &[yarlg::CharacterGroup] =
    &[group!(SPACE, ExactlyOne), group!(NULL, ExactlyOne)];

const WORD: Token =
    Token::new("WORD", &[group!(LOWER, AtLeastOne)], DELIMITERS);
const IF: Token = Token::new(
    "IF",
    &[group!(I, ExactlyOne), group!(F, ExactlyOne)],
    DELIMITERS,
);

const WORD_FIRST: &[Token] = &[WORD, IF];
const IF_FIRST: &[Token] = &[IF, WORD];

table_init!(LEXER_WORD_FIRST, WORD_FIRST);
table_init!(LEXER_IF_FIRST, IF_FIRST);

fn tok(label: &'static str, text: &str) -> (&'static str, String) {
    (label, text.to_string())
}

#[test]
fn keyword_declared_last_wins() {
    assert_eq!(
        Lexer::run("if iff i is", &LEXER_WORD_FIRST),
        Ok(vec![
            tok("IF", "if"),
            tok("WORD", "iff"),
            tok("WORD", "i"),
            tok("WORD", "is"),
        ])
    );
}

#[test]
fn word_declared_last_wins() {
    assert_eq!(
        Lexer::run("if iff", &LEXER_IF_FIRST),
        Ok(vec![tok("WORD", "if"), tok("WORD", "iff")])
    );
}

#[test]
fn rejects_undefined_characters() {
    assert!(Lexer::run("If", &LEXER_WORD_FIRST).is_err());
    assert!(Lexer::run("if1", &LEXER_WORD_FIRST).is_err());
}
