# yarlg

<p align="center">
<i>Blazingly fast Lexer generator.</i>
<p align="center">

**yarlg** allows you to make Lexers that are built entirely at compile time.

You define them with `lexer!`, a regex-like DSL. To build them it does the following:

- It turns Tokens defintions into a one single big DFA.
- It turns it into a 1D array that lives pre-compiled in memory where every possible state transitions is already pre-compiled.
- The resulting lexer looks up each UTF-8 byte directly in that array.

Each byte transition takes _O(1)_ time, so lexing _n_ input bytes takes _O(n)_ time.

## Quickstart

### Installing

```sh
cargo add yarlg
```

### Example

Let's define the following lexer:

```txt
NUMBER = {0...9}+
FLOAT = {0...9}+,".",{0...9}+
OP = "+","-","/","*"
EQ = "="
```

```rust
use yarlg::{Lexer, lexer};

// Create the Lexer, this runs entirely at compile time only.
lexer! {
    LEXER;
    // A fragment: a named class of characters to use in tokens, not a token.
    DIGIT = '0'..='9';

    NUMBER: DIGIT+;
    FLOAT:  DIGIT+ '.' DIGIT+;
    OP:     ('+' | '-' | '/' | '*');
    EQ:     '=';
}

fn main() {
    let tokens = Lexer::run("12 + 3.5 = 15.5", &LEXER).unwrap();

    println!("{:?}", tokens);
    // [("NUMBER", "12"), ("OP", "+"), ("FLOAT", "3.5"), ("EQ", "="), ("FLOAT", "15.5")]

    // Tokens don't need to be separated by whitespace.
    assert_eq!(Lexer::run("12+3.5=15.5", &LEXER).unwrap(), tokens);

    assert!(Lexer::run("1 % 2", &LEXER).is_err()); // '%' is not part of any token
}
```

See [More examples](#more-examples) for richer grammars and more nuanced examples.

## Syntax

```txt
lexer! {
    [pub] NAME [, max_states = N];
    FRAGMENT = class;
    TOKEN: item item ... [@ delimited_by(class)];
}
```

| Syntax          | Matches                                         |
| --------------- | ----------------------------------------------- |
| `'a'`           | the character `a`                               |
| `'a'..='z'`     | one character from `a` to `z`                   |
| `DIGIT`         | one character of the fragment `DIGIT`           |
| `('a' \| DIGIT)` | one character of any of the alternatives        |
| `"let"`         | `l`, `e` and `t`, in this order                 |
| `item+`         | `item` one or more times                        |
| `item*`         | `item` zero or more times                       |
| `eof`           | the end of input, in a class                    |

- A **class** is one or more alternatives separated by `|`, without parentheses in a fragment: `LETTER = 'a'..='z' | 'A'..='Z' | '_';`.
- When two tokens match the same input the **last declared** one wins. So keywords are declared after the identifiers.
- `pub NAME` makes the lexer public. `max_states = N` raises the number of DFA states the lexer can use (1024 by default).

## Delimiters

A token can end in two ways.

**By itself.** A token without `@ delimited_by(...)` ends as soon as the next character can't continue it. That character is not consumed, it's the start of the next token: `12+34` is `12`, `+`, `34`.

yarlg finds which characters end each token at compile time, looking at all the tokens together. So the longest match wins: with `ASSIGN: '='` and `EQ: "=="`, `a==b` is `a`, `==`, `b`. With `IDENT` and `LET: "let"`, `letter` is one `IDENT`.

What can continue a token may depend on what came before it. Next to `HEX: "0x" DIGIT+`, `0x1` is a `HEX`, while `10x` is a `NUMBER` followed by `x`. yarlg handles this for the first 4 characters of a token. After that, a token ends only on the characters that would end it whatever came before.

**With `@ delimited_by(class)`.** The token ends only before one of those characters, which is consumed and is not part of the token. This allows the delimiter to decide the label:

```rust
use yarlg::{Lexer, lexer};

lexer! {
    CONFIG;
    WORD = 'a'..='z' | '0'..='9' | '_' | '.';

    KEY:   WORD+ @ delimited_by('=');
    VALUE: WORD+ @ delimited_by('\n' | eof);
}

fn main() {
    let tokens = Lexer::run("host=127.0.0.1\nport=5432", &CONFIG).unwrap();

    println!("{:?}", tokens);
    // [("KEY", "host"), ("VALUE", "127.0.0.1"), ("KEY", "port"), ("VALUE", "5432")]
}
```

Whitespace (`' '`, `'\t'`, `'\n'`, `'\r'`) between tokens is skipped, unless a token can start with it.

## Compile-time errors

The lexer is a `const`, so an invalid definition fails compilation instead of panicking at runtime:

```txt
error[E0080]: evaluation panicked: Error: a token without `@ delimited_by(...)` must match at least one character.
```

- A token without `@ delimited_by(...)` must match at least one character (`DIGITS: DIGIT*;` fails).
- A token without `@ delimited_by(...)` must be able to end: if every character after it, end of input included, continues some token, compilation fails.

  In both cases, give the token `@ delimited_by(...)`.
- Ranges must be ordered: `'z'..='a'` fails.
- A lexer with tokens without `@ delimited_by(...)` can have at most 128 tokens, with at most 63 items each.
- When the lexer needs more states than `max_states` allows, increase it: `lexer! { LEXER, max_states = 4096; ... }`.
- Very large lexers (more than ~100 rules, not counting `NAME: "literal";` rules which are cheaper) may need a higher `#![recursion_limit]`. Around 50 overlapping rules can trip the `long_running_const_eval` lint, which `#![allow(long_running_const_eval)]` silences.

## More examples

### A small language

```rust
use yarlg::{Lexer, lexer};

lexer! {
    SCRIPT;
    LETTER = 'a'..='z' | 'A'..='Z' | '_';
    DIGIT  = '0'..='9';
    // Every character from ' ' except '"'.
    CHAR   = ' '..='!' | '#'..='\u{10FFFF}';

    IDENT:  LETTER (LETTER | DIGIT)*;
    // Keywords are declared after IDENT, so they win on the same input.
    LET: "let";  IF: "if";  ELSE: "else";
    INT:    DIGIT+;
    STRING: '"' CHAR* '"';
    ASSIGN: '=';  EQ: "==";
    LPAREN: '(';  RPAREN: ')';  SEMI: ';';
}

fn main() {
    let tokens = Lexer::run("let letter=\"ciao mondo\";if(letter==x1)", &SCRIPT).unwrap();

    println!("{:?}", tokens);
    // [("LET", "let"), ("IDENT", "letter"), ("ASSIGN", "="), ("STRING", "\"ciao mondo\""),
    //  ("SEMI", ";"), ("IF", "if"), ("LPAREN", "("), ("IDENT", "letter"), ("EQ", "=="),
    //  ("IDENT", "x1"), ("RPAREN", ")")]

    assert!(Lexer::run("\"open", &SCRIPT).is_err()); // the string never ends
}
```

### Numbers next to hexadecimals

After `0`, an `x` continues a `HEX`. After any other number, it starts an `IDENT`.

```rust
use yarlg::{Lexer, lexer};

lexer! {
    NUMBERS;
    DIGIT  = '0'..='9';
    LETTER = 'a'..='z';

    NUMBER: DIGIT+;
    HEX:    "0x" DIGIT+;
    IDENT:  LETTER+;
}

fn main() {
    let tokens = Lexer::run("0x12 0 10x", &NUMBERS).unwrap();

    println!("{:?}", tokens);
    // [("HEX", "0x12"), ("NUMBER", "0"), ("NUMBER", "10"), ("IDENT", "x")]
}
```

### Config files with comments

```rust
use yarlg::{Lexer, lexer};

lexer! {
    CONFIG;
    WORD = 'a'..='z' | '0'..='9' | '_' | '.';
    TEXT = '\t' | ' '..='\u{10FFFF}';
    END  = '\n' | eof;

    COMMENT: '#' TEXT* @ delimited_by(END);
    KEY:     WORD+     @ delimited_by('=');
    VALUE:   WORD+     @ delimited_by(END);
}

fn main() {
    let tokens = Lexer::run("# database\nhost=127.0.0.1\nport=5432", &CONFIG).unwrap();

    println!("{:?}", tokens);
    // [("COMMENT", "# database"), ("KEY", "host"), ("VALUE", "127.0.0.1"),
    //  ("KEY", "port"), ("VALUE", "5432")]

    assert!(Lexer::run("host = x", &CONFIG).is_err()); // ' ' is not part of KEY
}
```

### CSV rows

```rust
use yarlg::{Lexer, lexer};

lexer! {
    CSV;
    CELL_CHAR = 'a'..='z' | '0'..='9' | ' ';

    CELL:    CELL_CHAR+ @ delimited_by(',');
    ROW_END: CELL_CHAR+ @ delimited_by('\n' | eof);
}

fn main() {
    let tokens = Lexer::run("alice,30,rome\nbob,25,new york", &CSV).unwrap();

    println!("{:?}", tokens);
    // [("CELL", "alice"), ("CELL", "30"), ("ROW_END", "rome"),
    //  ("CELL", "bob"), ("CELL", "25"), ("ROW_END", "new york")]
}
```

## Without the DSL

`lexer!` is syntactic sugar: every rule is turned at compile time into `CharacterRange`, `CharacterGroup` and `Token`, then into `table_init!`. You can also write them by hand. This is the lexer of the [Quickstart](#example):

```rust
use yarlg::{
    CharacterRange as range,
    CharacterGroupType, 
    Lexer, 
    Token, 
    group, 
    table_init
};

// Define all ranges we need.
const DIGIT:    range = range::new().with_start('0').with_end('9').build();
const DOT:      range = range::new().with_start('.').build();
const PLUS:     range = range::new().with_start('+').build();
const MINUS:    range = range::new().with_start('-').build();
const SLASH:    range = range::new().with_start('/').build();
const STAR:     range = range::new().with_start('*').build();
const EQUAL:    range = range::new().with_start('=').build();
const SPACE:    range = range::new().with_start(' ').build();
const NULL:     range = range::new().with_start('\0').build();

// Put together ranges into groups and groups into tokens.
//
// SPACE & NULL (end of input) are common delimiters for every token,
// but you can define anything you want as a delimiter.
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
        "FLOAT",
        &[
            group!(DIGIT, CharacterGroupType::AtLeastOne),
            group!(DOT, CharacterGroupType::ExactlyOne),
            group!(DIGIT, CharacterGroupType::AtLeastOne),
        ],
        &[
            group!(SPACE, CharacterGroupType::ExactlyOne),
            group!(NULL, CharacterGroupType::ExactlyOne),
        ],
    ),
    Token::new(
        "OP",
        &[group!(PLUS MINUS SLASH STAR, CharacterGroupType::ExactlyOne)],
        &[
            group!(SPACE, CharacterGroupType::ExactlyOne),
            group!(NULL, CharacterGroupType::ExactlyOne),
        ],
    ),
    Token::new(
        "EQ",
        &[group!(EQUAL, CharacterGroupType::ExactlyOne)],
        &[
            group!(SPACE, CharacterGroupType::ExactlyOne),
            group!(NULL, CharacterGroupType::ExactlyOne),
        ],
    ),
];

// Create the Lexer, this runs entirely at compile time only.
table_init!(LEXER, TOKENS);

fn main() {
    let tokens = Lexer::run("12 + 3.5 = 15.5", &LEXER).unwrap();

    println!("{:?}", tokens);
    // [("NUMBER", "12"), ("OP", "+"), ("FLOAT", "3.5"), ("EQ", "="), ("FLOAT", "15.5")]

    assert!(Lexer::run("1 % 2", &LEXER).is_err()); // '%' is not part of any token
    assert!(Lexer::run("1+2", &LEXER).is_err()); // tokens must be separated by ' '
}
```

Hand-written delimiters are consumed. `Token::peek_delimiters()` keeps them in the input, so that `1+2` can be lexed too, but then you list what may follow every token yourself: `lexer!` does it for you.

> [!IMPORTANT]
> The end of the input is signalled with a trailing `'\0'`. Unless you know what you are doing always set 
> `'\0'` as a delimiter for every token you define.

## Unicode rules and byte transitions

`CharacterRange` takes Rust `char` literals. For example, `range::new().with_start('α').with_end('ω').build()` matches an inclusive range of Unicode scalar values. Repetition and delimiters operate on whole scalars, not on individual bytes or grapheme clusters. Matching does not normalize the input: `'é'` and `"e\u{301}"` are different sequences.

At compile time, yarlg translates these ranges into UTF-8 byte patterns and combines them into a DFA. Each state has 256 entries, including invalid transitions. A multibyte character takes two to four transitions; the table does not need a column for every Unicode scalar. At runtime, each input byte requires one lookup:

```rust
table[state as usize * 256 + byte as usize]
```

## Docs

- [**docs**](https://paoloanzn.github.io/yarlg/)

## License

[MIT](LICENSE)
