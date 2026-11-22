# yarlg

<p align="center">
<i>Blazingly fast Lexer generator.</i>
<p align="center">

**yarlg** allows you to make Lexers that are built entirely at compile time.

To achieve this it does the following:

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
