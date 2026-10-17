# yarlg

> Yet another Rust lexer generator.

`yarlg` is a **compile time** DFA lexer generator. The token definitions, the state machine and its transition table are all `const`, so the generated lexer is already a static array baked into the binary, and lexing is one array lookup per input character, O(1) time complexity for a single character transition and O(n) for a full input processing, where n is the number of characters of the input. Furthermore the crate has **zero** dependencies.

> [!WARNING]
> This version is still not 100% polished, expect possible errors, see [Limitations](#limitations). PRs are welcomed.

---

## TL;DR
- [Installation](#installation)
- [Usage](#usage)
- [How it works](#how-it-works)
- [Limitations](#limitations)
- [Docs](#docs)

---

## Installation

```sh
cargo add yarlg
```

or add it to your `Cargo.toml` manually:

```toml
[dependencies]
yarlg = "0.1"
```

`yarlg` needs **Rust 1.85** or newer.

## Usage

> Describe ranges, group them, build tokens out of groups, then let `table_init!` compile the whole thing into a lexer.

A lexer that reads numbers and lowercase identifiers separated by spaces:

```rust
use yarlg::{AsciiCharacterRange, CharacterGroupType, Lexer, Token, group, table_init};

const DIGIT: AsciiCharacterRange = AsciiCharacterRange::new().with_start(b'0').with_end(b'9').build();
const LOWER: AsciiCharacterRange = AsciiCharacterRange::new().with_start(b'a').with_end(b'z').build();
const SPACE: AsciiCharacterRange = AsciiCharacterRange::new().with_start(b' ').build();
const NULL: AsciiCharacterRange = AsciiCharacterRange::new().with_start(b'\0').build();

// NUMBER {'0'...'9'}+ followed by ' ' or end of input
// IDENT  {'a'...'z'}+ followed by ' ' or end of input
const TOKENS: &[Token] = &[
    Token::new(
        "NUMBER",
        &[group!(DIGIT, CharacterGroupType::AtLeastOne)],
        &[group!(SPACE, CharacterGroupType::ExactlyOne), group!(NULL, CharacterGroupType::ExactlyOne)],
    ),
    Token::new(
        "IDENT",
        &[group!(LOWER, CharacterGroupType::AtLeastOne)],
        &[group!(SPACE, CharacterGroupType::ExactlyOne), group!(NULL, CharacterGroupType::ExactlyOne)],
    ),
];

table_init!(LEXER, TOKENS);

fn main() {
    let tokens = Lexer::run("42 foo 7", &LEXER).unwrap();
    println!("{:?}", tokens);
    // [("NUMBER", "42"), ("IDENT", "foo"), ("NUMBER", "7")]

    assert!(Lexer::run("42 Foo", &LEXER).is_err()); // 'F' is not part of any token
}
```

There are only four building blocks:

- `AsciiCharacterRange`: a contiguous range of ASCII characters, `'0'...'9'`, or a single one, `'a'`.
- `group!`: a set of ranges with a type, `ExactlyOne` `{}`, `AtLeastOne` `{}+` or `ZeroOrMore` `{}*`. `group!(DIGIT LOWER, CharacterGroupType::AtLeastOne)` is `{'0'...'9', 'a'...'z'}+`.
- `Token`: a label, the groups the token is made of **in order**, and the delimiter groups that end it. The delimiter is consumed but not returned in the token text.
- `table_init!(NAME, TOKENS)`: defines a `const` transition table called `NAME`, ready for `Lexer::run`.

`Lexer::run` returns the tokens as `(label, text)` pairs, or an error at the first character no token accepts.

> [!IMPORTANT]
> The end of the input is signalled with a trailing `'\0'`. If a token can be the last thing in the input, give it a delimiter that matches `'\0'` (the `NULL` range in the example), otherwise input like `"42 foo"` is rejected and only `"42 foo "` is accepted.

## How it works

> The lexer goes like this: look at the char, look at the table & get the state, either fails, emit token and/or go on to the next char until the whole input is consumed.

### Tokens

A range $r$ is an interval of ASCII codes:

$$
r = [a, b] \subseteq \{0, \dots, 127\}, \qquad a \leq b
$$

where $a = b$ is a single character. A group $g = (R, \alpha)$ is a finite set of ranges $R$, matching any character in $\bigcup_{r \in R} r$, together with a repetition $\alpha \in \{1, +, *\}$: exactly one, at least one, or zero or more of those characters.

A token is the triple

$$
T = (L, G, D)
$$

Where:

- $L$ is a label, a unique name for the token.
- $G = (g_1, \dots, g_n)$ is an **ordered** sequence of groups, the body of the token.
- $D$ is a finite set of delimiter groups; $\alpha = *$ is not allowed here, since a token always needs a delimiter to end.

so the input matches $T$ when it matches $g_1$ then $g_2$ up to $g_n$, followed by one delimiter $d \in D$.

### The DFA

All the tokens together compile to a single DFA, the 5-tuple

$$
M = (Q, \Sigma, \delta, q_0, F)
$$

Where:

- $Q$ is a finite set of states.
- $\Sigma$ is a finite set of input symbols, the entire allowed charset (7-bit ASCII).
- $\delta$ is the transition function, also known as the **transition table**:

$$
\delta : Q \times \Sigma \rightarrow Q, \qquad \delta(q_1, s) = q_2
$$

- $q_0 \in Q$ is the start state, shared by every token.
- $F \subseteq Q$ is the set of final (accepting) states, one per token.

Each token adds its own states after $q_0$: one for every group with $\alpha = 1$ or $\alpha = +$ ($+$ also loops on itself), none for $\alpha = *$ (it loops on the state it starts from), and one final state reached through the delimiters. Any $(q, s)$ pair with no transition goes to the invalid state, which is a lexing error. Final states are stored as negative numbers, so checking whether a token was just accepted is a sign check, not a lookup.

### Transition function in O(1)

$\delta$ is stored as a flat 1-dimensional array, one row of 128 entries (one per ASCII character) for each state:

$$
\mathrm{max\_len} = |Q| \cdot 128
$$

$$
\mathrm{idx}(q, s) = q \cdot 128 + s, \qquad 0 \leq \mathrm{idx} < \mathrm{max\_len}
$$

The array is first filled with the invalid state, then every token writes only its own valid transitions. All of this runs in `const fn`s driven by `table_init!`, which also works out $|Q|$ and the number of tokens from the definitions, so the size of the array is known at compile time and running a transition is just:

```rust
table[state as usize * 128 + c as usize]
```

No branches on the character class, no allocations: one character in, one array read, next state.

## Limitations

- Only ASCII input is supported (characters `0..=127`).
- A single token can be at most 4096 characters long.
- The end of the input must match a `'\0'` delimiter, see [Usage](#usage).

## Docs

- [**API docs**](https://paoloanzn.github.io/yarlg/)
- [**Tests**](tests/lexer.rs)

---

## License

[MIT](LICENSE)
