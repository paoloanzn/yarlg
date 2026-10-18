use yarlg::{
    AsciiCharacterRange, CharacterGroupType, Lexer, Token, group, table_init,
};

// An integer literals lexer:
// DEC = {0...9}+
// HEX = "0","x",{0...9, a...f}+
// BIN = "0","b",{0, 1}+
// every token followed by ' ' or end of input.
// All three tokens can start with '0', so they share the same first state.
const DIGIT: AsciiCharacterRange = AsciiCharacterRange::new()
    .with_start(b'0')
    .with_end(b'9')
    .build();
const HEX_LETTER: AsciiCharacterRange = AsciiCharacterRange::new()
    .with_start(b'a')
    .with_end(b'f')
    .build();
const BIT: AsciiCharacterRange = AsciiCharacterRange::new()
    .with_start(b'0')
    .with_end(b'1')
    .build();
const ZERO: AsciiCharacterRange =
    AsciiCharacterRange::new().with_start(b'0').build();
const X: AsciiCharacterRange =
    AsciiCharacterRange::new().with_start(b'x').build();
const B: AsciiCharacterRange =
    AsciiCharacterRange::new().with_start(b'b').build();
const SPACE: AsciiCharacterRange =
    AsciiCharacterRange::new().with_start(b' ').build();
const NULL: AsciiCharacterRange =
    AsciiCharacterRange::new().with_start(b'\0').build();

const TOKENS: &[Token] = &[
    Token::new(
        "DEC",
        &[group!(DIGIT, CharacterGroupType::AtLeastOne)],
        &[
            group!(SPACE, CharacterGroupType::ExactlyOne),
            group!(NULL, CharacterGroupType::ExactlyOne),
        ],
    ),
    Token::new(
        "HEX",
        &[
            group!(ZERO, CharacterGroupType::ExactlyOne),
            group!(X, CharacterGroupType::ExactlyOne),
            group!(DIGIT HEX_LETTER, CharacterGroupType::AtLeastOne),
        ],
        &[
            group!(SPACE, CharacterGroupType::ExactlyOne),
            group!(NULL, CharacterGroupType::ExactlyOne),
        ],
    ),
    Token::new(
        "BIN",
        &[
            group!(ZERO, CharacterGroupType::ExactlyOne),
            group!(B, CharacterGroupType::ExactlyOne),
            group!(BIT, CharacterGroupType::AtLeastOne),
        ],
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
fn lexes_mixed_literals() {
    assert_eq!(
        lex("255 0xff 0b11111111").unwrap(),
        vec![
            tok("DEC", "255"),
            tok("HEX", "0xff"),
            tok("BIN", "0b11111111"),
        ]
    );
}

#[test]
fn zero_alone_is_decimal() {
    assert_eq!(lex("0").unwrap(), vec![tok("DEC", "0")]);
}

#[test]
fn decimal_with_leading_zeros() {
    // HEX and BIN merged into the '0' state, DEC must still loop on digits.
    for s in ["00", "007", "0123456789"] {
        assert_eq!(lex(s).unwrap(), vec![tok("DEC", s)], "input {s:?}");
    }
}

#[test]
fn hex_digits() {
    for s in ["0x0", "0xdeadbeef", "0x1a2b3c", "0xb"] {
        assert_eq!(lex(s).unwrap(), vec![tok("HEX", s)], "input {s:?}");
    }
}

#[test]
fn bin_digits() {
    for s in ["0b0", "0b1", "0b1010"] {
        assert_eq!(lex(s).unwrap(), vec![tok("BIN", s)], "input {s:?}");
    }
}

#[test]
fn all_prefixes_in_sequence() {
    assert_eq!(
        lex("0 0x0 0b0 0").unwrap(),
        vec![
            tok("DEC", "0"),
            tok("HEX", "0x0"),
            tok("BIN", "0b0"),
            tok("DEC", "0"),
        ]
    );
}

#[test]
fn rejects_prefix_without_digits() {
    assert!(lex("0x").is_err());
    assert!(lex("0b").is_err());
    assert!(lex("0x ").is_err());
}

#[test]
fn rejects_prefix_not_after_zero() {
    assert!(lex("1x5").is_err());
    assert!(lex("x5").is_err());
}

#[test]
fn rejects_invalid_hex_digit() {
    assert!(lex("0xg").is_err());
    assert!(lex("0xFF").is_err());
}

#[test]
fn rejects_invalid_bin_digit() {
    assert!(lex("0b102").is_err());
}

#[test]
fn rejects_uppercase_prefix() {
    assert!(lex("0X1").is_err());
    assert!(lex("0B1").is_err());
}
