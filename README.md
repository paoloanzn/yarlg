# yarlg

<p align="center">
<i>Blazingly fast Lexer generator.</i>
<p align="center">

**yarlg** allows you to make Lexers that are built entirely at compile time.

To achieve this it does the following:

- It turns Tokens defintions into a one single big DFA.
- It turns it into a 1D array that lives pre-compiled in memory where every possible state transitions is already pre-compiled.
- The resulting lexer process every character of the input string by doing a direct look up on that array.

As a result yarlg turns every single character processing into an _O(1)_ operation and the complete input processing into an _O(n)_ operation where _n_ is the lenght of the input.

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
    AsciiCharacterRange as range, 
    CharacterGroupType, 
    Lexer, 
    Token, 
    group, 
    table_init
};

// Define all ranges we need.
const DIGIT:    range = range::new().with_start('0' as u8).with_end('9' as u8).build();
const DOT:      range = range::new().with_start('.' as u8).build();
const PLUS:     range = range::new().with_start('+' as u8).build();
const MINUS:    range = range::new().with_start('-' as u8).build();
const SLASH:    range = range::new().with_start('/' as u8).build();
const STAR:     range = range::new().with_start('*' as u8).build();
const EQUAL:    range = range::new().with_start('=' as u8).build();
const SPACE:    range = range::new().with_start(' ' as u8).build();
const NULL:     range = range::new().with_start('\0' as u8).build();

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

### Know Limitations

- Only ASCII input is supported (characters `0..=127`).

## Yarlg fast approach

yarlg Lexer generation is based around a fixed _O(1)_ transition function. All the possible transtions
for all possible states of the Lexer are stored as a flat 1-dimensional array, one row of 128 entries (one per ASCII character) for each state.

The array is first filled with the invalid state, then every token writes only its own valid transitions. All of this runs in `const fn` compile time function, meaning the table is built-in in the final binary itself. Nothing
is done at runtime except for a simple table lookup:

```rust
table[state as usize * 128 + c as usize]
```

## Docs

- [**docs**](https://paoloanzn.github.io/yarlg/)

## License

[MIT](LICENSE)
