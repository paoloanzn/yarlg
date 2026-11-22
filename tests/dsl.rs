use yarlg::{
    CharacterGroupType::*, CharacterRange as range, Lexer, Token, group, lexer,
    table_init,
};

fn tok(label: &'static str, text: &str) -> (&'static str, String) {
    (label, text.to_string())
}

// NUMBER = {0...9}+
// FLOAT  = {0...9}+,{.},{0...9}+
// The DSL must lower to the exact same table as the hand-written tokens.
lexer! {
    NUMBERS;
    DIGIT = '0'..='9';

    NUMBER: DIGIT+;
    FLOAT:  DIGIT+ '.' DIGIT+;
}

#[test]
fn same_table_as_hand_written_tokens() {
    const DIGIT: range = range::new().with_start('0').with_end('9').build();
    const DOT: range = range::new().with_start('.').build();
    const SPACE: range = range::new().with_start(' ').build();
    const TAB: range = range::new().with_start('\t').build();
    const NEWLINE: range = range::new().with_start('\n').build();
    const RETURN: range = range::new().with_start('\r').build();
    const NULL: range = range::new().with_start('\0').build();
    const DELIMITERS: &[yarlg::CharacterGroup] =
        &[group!(SPACE TAB NEWLINE RETURN NULL, ExactlyOne)];
    const TOKENS: &[Token] = &[
        Token::new("NUMBER", &[group!(DIGIT, AtLeastOne)], DELIMITERS),
        Token::new(
            "FLOAT",
            &[
                group!(DIGIT, AtLeastOne),
                group!(DOT, ExactlyOne),
                group!(DIGIT, AtLeastOne),
            ],
            DELIMITERS,
        ),
    ];
    table_init!(HAND_WRITTEN, TOKENS);

    assert_eq!(&NUMBERS.table[..], &HAND_WRITTEN.table[..]);
    assert_eq!(
        &NUMBERS.terminal_states[..],
        &HAND_WRITTEN.terminal_states[..]
    );
}

// Default delimiters: tokens are separated by whitespace.
#[test]
fn calculator() {
    lexer! {
        CALC;
        DIGIT = '0'..='9';

        NUMBER: DIGIT+;
        FLOAT:  DIGIT+ '.' DIGIT+;
        PLUS: '+';  MINUS: '-';  STAR: '*';  SLASH: '/';
        LPAREN: '(';  RPAREN: ')';
    }
    assert_eq!(
        Lexer::run("( 12 + 3.5 ) * 2", &CALC),
        Ok(vec![
            tok("LPAREN", "("),
            tok("NUMBER", "12"),
            tok("PLUS", "+"),
            tok("FLOAT", "3.5"),
            tok("RPAREN", ")"),
            tok("STAR", "*"),
            tok("NUMBER", "2"),
        ])
    );
    // No maximal munch yet: tokens need a delimiter.
    assert!(Lexer::run("(12+3.5)*2", &CALC).is_err());
}

// Keywords are declared after IDENT, so they win on the same input.
// CHAR is every character from ' ' except '"'.
#[test]
fn keywords_identifiers_and_strings() {
    lexer! {
        SCRIPT;
        LETTER = 'a'..='z' | 'A'..='Z' | '_';
        DIGIT  = '0'..='9';
        CHAR   = ' '..='!' | '#'..='\u{10FFFF}';

        IDENT:  LETTER (LETTER | DIGIT)*;
        LET: "let";  IF: "if";  ELSE: "else";
        INT:    DIGIT+;
        STRING: '"' CHAR* '"';
        ASSIGN: '=';
        EQ:     "==";
    }
    assert_eq!(
        Lexer::run("let greeting = \"ciao mondo\"", &SCRIPT),
        Ok(vec![
            tok("LET", "let"),
            tok("IDENT", "greeting"),
            tok("ASSIGN", "="),
            tok("STRING", "\"ciao mondo\""),
        ])
    );
    assert_eq!(
        Lexer::run("if x == 10 else", &SCRIPT),
        Ok(vec![
            tok("IF", "if"),
            tok("IDENT", "x"),
            tok("EQ", "=="),
            tok("INT", "10"),
            tok("ELSE", "else"),
        ])
    );
    assert_eq!(
        Lexer::run("letter = \"\"", &SCRIPT),
        Ok(vec![
            tok("IDENT", "letter"),
            tok("ASSIGN", "="),
            tok("STRING", "\"\""),
        ])
    );
    assert!(Lexer::run("\"open", &SCRIPT).is_err());
}

// KEY and VALUE have the same pattern: only the delimiter decides the label.
// The delimiters are consumed, so '=' never shows up as a token.
#[test]
fn config_with_comments() {
    lexer! {
        CONFIG;
        WORD = 'a'..='z' | '0'..='9' | '_' | '.';
        TEXT = '\t' | ' '..='\u{10FFFF}';
        END  = '\n' | eof;

        COMMENT: '#' TEXT* @ delimited_by(END);
        KEY:     WORD+     @ delimited_by('=');
        VALUE:   WORD+     @ delimited_by(END);
    }
    assert_eq!(
        Lexer::run("# database\nhost=127.0.0.1\nport=5432", &CONFIG),
        Ok(vec![
            tok("COMMENT", "# database"),
            tok("KEY", "host"),
            tok("VALUE", "127.0.0.1"),
            tok("KEY", "port"),
            tok("VALUE", "5432"),
        ])
    );
    assert_eq!(
        Lexer::run("\n# a=b\nk=v\n", &CONFIG),
        Ok(vec![
            tok("COMMENT", "# a=b"),
            tok("KEY", "k"),
            tok("VALUE", "v")
        ])
    );
    assert!(Lexer::run("host = x", &CONFIG).is_err());
}

// Without `eof` in the delimiters the ';' is mandatory.
#[test]
fn mandatory_terminator() {
    lexer! {
        CSS;
        NAME = 'a'..='z' | '-';
        VAL  = 'a'..='z' | 'A'..='Z' | '0'..='9' | '#' | '-' | '.' | '%';

        PROPERTY: NAME+ @ delimited_by(':');
        VALUE:    VAL+  @ delimited_by(';');
    }
    assert_eq!(
        Lexer::run("color: red;\nmargin-top: -4px;", &CSS),
        Ok(vec![
            tok("PROPERTY", "color"),
            tok("VALUE", "red"),
            tok("PROPERTY", "margin-top"),
            tok("VALUE", "-4px"),
        ])
    );
    assert!(Lexer::run("color: red", &CSS).is_err());
    assert!(Lexer::run("color : red;", &CSS).is_err());
}

// ' ' is part of the first group, so leading spaces are not skipped anymore.
#[test]
fn csv_rows() {
    lexer! {
        CSV;
        CELL_CHAR = 'a'..='z' | '0'..='9' | ' ';

        CELL:    CELL_CHAR+ @ delimited_by(',');
        ROW_END: CELL_CHAR+ @ delimited_by('\n' | eof);
    }
    assert_eq!(
        Lexer::run("alice,30,rome\nbob,25,new york", &CSV),
        Ok(vec![
            tok("CELL", "alice"),
            tok("CELL", "30"),
            tok("ROW_END", "rome"),
            tok("CELL", "bob"),
            tok("CELL", "25"),
            tok("ROW_END", "new york"),
        ])
    );
    assert_eq!(
        Lexer::run("alice, 30\n", &CSV),
        Ok(vec![tok("CELL", "alice"), tok("ROW_END", " 30")])
    );
}

// A delimiter always wins over continuing another token.
#[test]
fn delimiter_beats_continuation() {
    lexer! {
        EDGE;
        NAME: ('a'..='z')+       @ delimited_by(':');
        TEXT: ('a'..='z' | ':')+ @ delimited_by(eof);
    }
    assert_eq!(
        Lexer::run("a:b:c", &EDGE),
        Ok(vec![tok("NAME", "a"), tok("NAME", "b"), tok("TEXT", "c")])
    );
}

// Unicode in strings, ranges and quantified characters.
#[test]
fn unicode_rules() {
    lexer! {
        UNICODE;
        GREEK = 'α'..='ω';

        WORD:  GREEK+;
        ARROW: "→>";
        FACES: '😀'+;
    }
    // 'ό' (with tonos) is outside 'α'..='ω'.
    assert!(
        Lexer::run("λόγος →> 😀😀", &UNICODE)
            .unwrap_err()
            .contains('ό')
    );
    assert_eq!(
        Lexer::run("λογος →> 😀😀", &UNICODE),
        Ok(vec![
            tok("WORD", "λογος"),
            tok("ARROW", "→>"),
            tok("FACES", "😀😀"),
        ])
    );
}

// `pub` lexers can be used from other modules, `max_states` is forwarded.
mod lexers {
    use yarlg::lexer;

    lexer! {
        pub WORDS, max_states = 64;
        WORD: ('a'..='z')+;
    }
}

#[test]
fn public_lexer_with_max_states() {
    assert_eq!(
        Lexer::run("hello world", &lexers::WORDS),
        Ok(vec![tok("WORD", "hello"), tok("WORD", "world")])
    );
}

// Long rules are split over several steps.
#[test]
fn long_rules() {
    lexer! {
        LONG;
        HEX = '0'..='9' | 'a'..='f' | 'A'..='F';

        COLOR: '#' HEX HEX HEX HEX HEX HEX @ delimited_by(' ' | ';' | eof);
        UUID:  HEX HEX HEX HEX HEX HEX HEX HEX '-' HEX HEX HEX HEX '-' HEX HEX HEX HEX;
    }
    assert_eq!(
        Lexer::run("#ff00AA;0123abcd-4567-89ef", &LONG),
        Ok(vec![
            tok("COLOR", "#ff00AA"),
            tok("UUID", "0123abcd-4567-89ef")
        ])
    );
}
