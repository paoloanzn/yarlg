use yarlg::{
    CharacterGroupType::*, CharacterRange as range, Lexer, Token, group,
    table_init,
};

// WORD = {a...z}+
// IF   = {i},{f}
// "if" matches both tokens: the last declared one wins.
const LOWER: range = range::new().with_start('a').with_end('z').build();
const I: range = range::new().with_start('i').build();
const F: range = range::new().with_start('f').build();
const SPACE: range = range::new().with_start(' ').build();
const NULL: range = range::new().with_start('\0').build();

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

#[test]
fn unicode_keyword_and_delimiter_share_byte_prefixes() {
    const LETTERS: range = range::new().with_start('è').with_end('ë').build();
    const E: range = range::new().with_start('é').build();
    const SEP: range = range::new().with_end('ì').build();
    const END: &[yarlg::CharacterGroup] = &[group!(SEP NULL, ExactlyOne)];
    const WORDS: &[Token] = &[
        Token::new("WORD", &[group!(LETTERS, AtLeastOne)], END),
        Token::new(
            "PAIR",
            &[group!(E, ExactlyOne), group!(E, ExactlyOne)],
            END,
        ),
    ];
    table_init!(UNICODE, WORDS);
    assert_eq!(
        Lexer::run("  ééìèêëìééé", &UNICODE),
        Ok(vec![
            tok("PAIR", "éé"),
            tok("WORD", "èêë"),
            tok("WORD", "ééé")
        ])
    );
    assert!(Lexer::run("éí", &UNICODE).unwrap_err().contains('í'));
}

#[test]
fn overlapping_repetitions_reuse_merged_rows() {
    const E: range = range::new().with_start('é').build();
    const RULES: &[Token] = &[Token::new(
        "THREE_OR_MORE",
        &[
            group!(E, AtLeastOne),
            group!(E, AtLeastOne),
            group!(E, AtLeastOne),
        ],
        DELIMITERS,
    )];
    table_init!(UNICODE, RULES, max_states = 64);
    assert!(Lexer::run("éé", &UNICODE).is_err());
    assert_eq!(
        Lexer::run("ééé ééééé", &UNICODE),
        Ok(vec![
            tok("THREE_OR_MORE", "ééé"),
            tok("THREE_OR_MORE", "ééééé")
        ])
    );
}
