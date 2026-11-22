//! The `lexer!` DSL.
//!
//! `lexer!` is syntactic sugar over [`Token`], [`CharacterGroup`] and `table_init!`:
//! every rule is lowered at compile time into the same structures you would
//! write by hand, so the resulting table is identical.
//!
//! A lexer goes through three steps, each one a `const` in the lexer's module:
//!
//! 1. Parsing (`__lexer!`, macro time). `@split` cuts the input at every `;`
//!    (up to 16 tokens per step, which keeps the macro recursion depth close to
//!    the number of rules). Every rule is then expanded side by side: fragments
//!    become `const` ranges, tokens are collected **in order** into `__RAW`.
//!    A token without `@ delimited_by(...)` gets no delimiters yet.
//! 2. Planning (`Plan::new`, compile time). Every token without delimiters is
//!    analyzed against all the others to find where it ends, see
//!    "Automatic delimiters" below. The result is a `Plan`: the final tokens,
//!    described with indexes.
//! 3. Lowering (`plan_*`, compile time). The plan becomes real `Token`s in
//!    `__TOKENS`, which `table_init!` turns into the lexer table.

use crate::{CharacterGroup, CharacterGroupType, CharacterRange, Token};

/// Defines a lexer with a regex-like grammar and creates its
/// [`TransitionTable`](crate::TransitionTable), computed entirely at compile time.
///
/// ```text
/// lexer! {
///     [pub] NAME [, max_states = N];
///     FRAGMENT = class;
///     TOKEN: item item ... [@ delimited_by(class)];
/// }
/// ```
///
/// + A **class** is one or more alternatives separated by `|`: a character `'a'`,
///   a range `'a'..='z'`, a `FRAGMENT` or `eof` (the end of input).
/// + An **item** is a character, a range, a `FRAGMENT` or a `(class)`, optionally
///   followed by `+` (one or more) or `*` (zero or more). A `"string"` matches
///   its characters in order.
/// + A token without `@ delimited_by(...)` ends by itself, as soon as the next
///   character can't continue it (nor any other token that matches the same
///   input). That character is not consumed: `12+34` is `12`, `+`, `34`.
///   What may continue can depend on what was read: next to `HEX: "0x" DIGIT+`,
///   `0x1` is `HEX` but `10x` is `NUMBER`, `x`. This is looked at for the first
///   4 characters of a token, after that it ends only on the characters that
///   would end it whatever came before.
/// + A token with `@ delimited_by(class)` ends only before one of those
///   characters, which is consumed and is not part of the token text.
/// + When two tokens match the same input the **last declared** one wins.
///
/// # Examples
///
/// ```
/// use yarlg::{Lexer, lexer};
///
/// lexer! {
///     CALC;
///     DIGIT = '0'..='9';
///
///     NUMBER: DIGIT+;
///     PLUS:   '+';
/// }
///
/// let tokens = Lexer::run("12+34", &CALC).unwrap();
/// assert_eq!(
///     tokens,
///     vec![("NUMBER", "12".into()), ("PLUS", "+".into()), ("NUMBER", "34".into())]
/// );
/// ```
///
/// The delimiter decides the label:
///
/// ```
/// use yarlg::{Lexer, lexer};
///
/// lexer! {
///     CONFIG;
///     WORD = 'a'..='z' | '0'..='9' | '_' | '.';
///
///     KEY:   WORD+ @ delimited_by('=');
///     VALUE: WORD+ @ delimited_by('\n' | eof);
/// }
///
/// let tokens = Lexer::run("host=127.0.0.1\nport=5432", &CONFIG).unwrap();
/// assert_eq!(
///     tokens,
///     vec![
///         ("KEY", "host".into()),
///         ("VALUE", "127.0.0.1".into()),
///         ("KEY", "port".into()),
///         ("VALUE", "5432".into()),
///     ]
/// );
/// ```
///
/// # Panics
///
/// Like [`table_init!`](crate::table_init), an invalid definition fails compilation.
/// A token without `@ delimited_by(...)` also fails it when:
///
/// + it can match an empty input;
/// + every character after it continues some token, so it would never end.
///
/// Give those tokens `@ delimited_by(...)`.
///
/// Very large lexers (more than ~100 rules, not counting `NAME: "literal";`
/// rules which are cheaper) may need a higher `#![recursion_limit]`.
#[macro_export]
macro_rules! lexer {
    ($vis:vis $name:ident $(, max_states = $max:expr)? ; $($rules:tt)*) => {
        // Fragments and tokens live in a hidden module named after the lexer,
        // modules and consts don't share a namespace so the names don't clash.
        #[doc(hidden)]
        #[allow(non_snake_case, non_upper_case_globals)]
        mod $name {
            $crate::__lexer! { @split [] [] $($rules)* }
        }
        $crate::table_init!($vis $name, $name::__TOKENS $(, max_states = $max)?);
    };
    ($($rest:tt)*) => {
        compile_error!("expected `lexer! { [pub] NAME [, max_states = N]; rules... }`");
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __lexer {
    // Split the rules at `;`. Each arm closes a rule found within the next 16 tokens,
    // the last one moves 16 tokens (none of them is a `;`) into the current rule.
    //
    // Keywords and operators (`NAME: "literal";`) are the most common rules,
    // so they are closed 4 at a time.
    (@split [$($r:tt)*] []
        $a:ident : $b:literal ; $c:ident : $d:literal ;
        $e:ident : $f:literal ; $g:ident : $h:literal ; $($rest:tt)*
    ) => {
        $crate::__lexer! { @split [$($r)* [$a : $b] [$c : $d] [$e : $f] [$g : $h]] [] $($rest)* }
    };
    (@split [$($r:tt)*] []) => {
        $( $crate::__lexer! { @fragment $r } )*
        // Step 1: the tokens as written.
        const __RAW: &[$crate::Token<'static>] =
            $crate::__lexer! { @tokens $( ($crate::__lexer! { @token $r }) )* };
        // Step 2: where every token ends.
        const __PLAN: $crate::dsl::Plan = $crate::dsl::Plan::new(__RAW);
        // Step 3: the final tokens. Their groups and delimiters are slices of
        // `__RANGES`, so they live in consts too.
        const __RANGES: &[$crate::CharacterRange] = const {
            &$crate::dsl::plan_ranges::<{ __PLAN.ranges_len() }>(&__PLAN)
        };
        const __GROUPS: &[$crate::CharacterGroup<'static>] = const {
            &$crate::dsl::plan_groups::<{ __PLAN.groups_len() }>(&__PLAN, __RAW, __RANGES)
        };
        const __DELIMITERS: &[$crate::CharacterGroup<'static>] = const {
            &$crate::dsl::plan_delimiters::<{ __PLAN.tokens_len() }>(&__PLAN, __RANGES)
        };
        pub(super) const __TOKENS: &[$crate::Token<'static>] = const {
            &$crate::dsl::plan_tokens::<{ __PLAN.tokens_len() }>(&__PLAN, __RAW, __GROUPS, __DELIMITERS)
        };
    };
    (@split [$($r:tt)*] [$($c:tt)*] ; $($rest:tt)*) => {
        $crate::__lexer! { @split [$($r)* [$($c)*]] [] $($rest)* }
    };
    (@split [$($r:tt)*] [$($c:tt)*] $t0:tt ; $($rest:tt)*) => {
        $crate::__lexer! { @split [$($r)* [$($c)* $t0]] [] $($rest)* }
    };
    (@split [$($r:tt)*] [$($c:tt)*] $t0:tt $t1:tt ; $($rest:tt)*) => {
        $crate::__lexer! { @split [$($r)* [$($c)* $t0 $t1]] [] $($rest)* }
    };
    (@split [$($r:tt)*] [$($c:tt)*] $t0:tt $t1:tt $t2:tt ; $($rest:tt)*) => {
        $crate::__lexer! { @split [$($r)* [$($c)* $t0 $t1 $t2]] [] $($rest)* }
    };
    (@split [$($r:tt)*] [$($c:tt)*] $t0:tt $t1:tt $t2:tt $t3:tt ; $($rest:tt)*) => {
        $crate::__lexer! { @split [$($r)* [$($c)* $t0 $t1 $t2 $t3]] [] $($rest)* }
    };
    (@split [$($r:tt)*] [$($c:tt)*] $t0:tt $t1:tt $t2:tt $t3:tt $t4:tt ; $($rest:tt)*) => {
        $crate::__lexer! { @split [$($r)* [$($c)* $t0 $t1 $t2 $t3 $t4]] [] $($rest)* }
    };
    (@split [$($r:tt)*] [$($c:tt)*] $t0:tt $t1:tt $t2:tt $t3:tt $t4:tt $t5:tt ; $($rest:tt)*) => {
        $crate::__lexer! { @split [$($r)* [$($c)* $t0 $t1 $t2 $t3 $t4 $t5]] [] $($rest)* }
    };
    (@split [$($r:tt)*] [$($c:tt)*] $t0:tt $t1:tt $t2:tt $t3:tt $t4:tt $t5:tt $t6:tt ; $($rest:tt)*) => {
        $crate::__lexer! { @split [$($r)* [$($c)* $t0 $t1 $t2 $t3 $t4 $t5 $t6]] [] $($rest)* }
    };
    (@split [$($r:tt)*] [$($c:tt)*] $t0:tt $t1:tt $t2:tt $t3:tt $t4:tt $t5:tt $t6:tt $t7:tt ; $($rest:tt)*) => {
        $crate::__lexer! { @split [$($r)* [$($c)* $t0 $t1 $t2 $t3 $t4 $t5 $t6 $t7]] [] $($rest)* }
    };
    (@split [$($r:tt)*] [$($c:tt)*] $t0:tt $t1:tt $t2:tt $t3:tt $t4:tt $t5:tt $t6:tt $t7:tt $t8:tt ; $($rest:tt)*) => {
        $crate::__lexer! { @split [$($r)* [$($c)* $t0 $t1 $t2 $t3 $t4 $t5 $t6 $t7 $t8]] [] $($rest)* }
    };
    (@split [$($r:tt)*] [$($c:tt)*] $t0:tt $t1:tt $t2:tt $t3:tt $t4:tt $t5:tt $t6:tt $t7:tt $t8:tt $t9:tt ; $($rest:tt)*) => {
        $crate::__lexer! { @split [$($r)* [$($c)* $t0 $t1 $t2 $t3 $t4 $t5 $t6 $t7 $t8 $t9]] [] $($rest)* }
    };
    (@split [$($r:tt)*] [$($c:tt)*] $t0:tt $t1:tt $t2:tt $t3:tt $t4:tt $t5:tt $t6:tt $t7:tt $t8:tt $t9:tt $t10:tt ; $($rest:tt)*) => {
        $crate::__lexer! { @split [$($r)* [$($c)* $t0 $t1 $t2 $t3 $t4 $t5 $t6 $t7 $t8 $t9 $t10]] [] $($rest)* }
    };
    (@split [$($r:tt)*] [$($c:tt)*] $t0:tt $t1:tt $t2:tt $t3:tt $t4:tt $t5:tt $t6:tt $t7:tt $t8:tt $t9:tt $t10:tt $t11:tt ; $($rest:tt)*) => {
        $crate::__lexer! { @split [$($r)* [$($c)* $t0 $t1 $t2 $t3 $t4 $t5 $t6 $t7 $t8 $t9 $t10 $t11]] [] $($rest)* }
    };
    (@split [$($r:tt)*] [$($c:tt)*] $t0:tt $t1:tt $t2:tt $t3:tt $t4:tt $t5:tt $t6:tt $t7:tt $t8:tt $t9:tt $t10:tt $t11:tt $t12:tt ; $($rest:tt)*) => {
        $crate::__lexer! { @split [$($r)* [$($c)* $t0 $t1 $t2 $t3 $t4 $t5 $t6 $t7 $t8 $t9 $t10 $t11 $t12]] [] $($rest)* }
    };
    (@split [$($r:tt)*] [$($c:tt)*] $t0:tt $t1:tt $t2:tt $t3:tt $t4:tt $t5:tt $t6:tt $t7:tt $t8:tt $t9:tt $t10:tt $t11:tt $t12:tt $t13:tt ; $($rest:tt)*) => {
        $crate::__lexer! { @split [$($r)* [$($c)* $t0 $t1 $t2 $t3 $t4 $t5 $t6 $t7 $t8 $t9 $t10 $t11 $t12 $t13]] [] $($rest)* }
    };
    (@split [$($r:tt)*] [$($c:tt)*] $t0:tt $t1:tt $t2:tt $t3:tt $t4:tt $t5:tt $t6:tt $t7:tt $t8:tt $t9:tt $t10:tt $t11:tt $t12:tt $t13:tt $t14:tt ; $($rest:tt)*) => {
        $crate::__lexer! { @split [$($r)* [$($c)* $t0 $t1 $t2 $t3 $t4 $t5 $t6 $t7 $t8 $t9 $t10 $t11 $t12 $t13 $t14]] [] $($rest)* }
    };
    (@split $r:tt [$($c:tt)*] $t0:tt $t1:tt $t2:tt $t3:tt $t4:tt $t5:tt $t6:tt $t7:tt $t8:tt $t9:tt $t10:tt $t11:tt $t12:tt $t13:tt $t14:tt $t15:tt $($rest:tt)*) => {
        $crate::__lexer! { @split $r [$($c)* $t0 $t1 $t2 $t3 $t4 $t5 $t6 $t7 $t8 $t9 $t10 $t11 $t12 $t13 $t14 $t15] $($rest)* }
    };
    (@split $r:tt [$($c:tt)*] $($rest:tt)*) => {
        compile_error!(concat!("expected `;` after `", stringify!($($c)* $($rest)*), "`"));
    };

    // A fragment is a `const` character class, a token expands to nothing here.
    (@fragment [$name:ident = $($class:tt)*]) => {
        const $name: &[$crate::CharacterRange] = $crate::__lexer! { @class [] | $($class)* };
    };
    (@fragment [$name:ident : $($items:tt)*]) => {};
    (@fragment [$($rule:tt)*]) => {
        compile_error!(concat!(
            "expected `NAME = class` or `NAME: pattern`, found `", stringify!($($rule)*), "`"
        ));
    };

    // A token is a one element `&[Token]`, a fragment an empty one.
    (@token [$name:ident : $($items:tt)*]) => {
        $crate::__lexer! { @items ($name) [] $($items)* }
    };
    (@token $rule:tt) => { &[] };

    (@tokens $($token:tt)*) => {
        const {
            &$crate::dsl::tokens::<{ $crate::dsl::count_tokens(&[$($token),*]) }>(&[$($token),*])
        }
    };

    // Class: `| alternative` repeated, every alternative is a `&[CharacterRange]`.
    (@class [$($part:tt)*]) => {
        const {
            &$crate::dsl::ranges::<{ $crate::dsl::count_ranges(&[$($part),*]) }>(&[$($part),*])
        }
    };
    (@class [$($part:tt)*] | $first:literal ..= $last:literal $($rest:tt)*) => {
        $crate::__lexer! { @class [$($part)* (&[$crate::dsl::range(concat!($first), concat!($last))])] $($rest)* }
    };
    (@class [$($part:tt)*] | eof $($rest:tt)*) => {
        $crate::__lexer! { @class [$($part)* (&[$crate::dsl::range("\0", "\0")])] $($rest)* }
    };
    (@class [$($part:tt)*] | $fragment:ident $($rest:tt)*) => {
        $crate::__lexer! { @class [$($part)* ($fragment)] $($rest)* }
    };
    (@class [$($part:tt)*] | $char:literal $($rest:tt)*) => {
        $crate::__lexer! { @class [$($part)* (&[$crate::dsl::range(concat!($char), concat!($char))])] $($rest)* }
    };
    (@class $parts:tt $($rest:tt)*) => {
        compile_error!(concat!("unexpected `", stringify!($($rest)*), "` in character class"))
    };

    // Items: every item is a `&[CharacterGroup]`, `"strings"` expand to one group per character.
    (@items ($name:ident) []) => {
        compile_error!(concat!("token `", stringify!($name), "` has an empty pattern"))
    };
    (@items ($name:ident) [] @ $($rest:tt)*) => {
        compile_error!(concat!("token `", stringify!($name), "` has an empty pattern"))
    };
    // No delimiters: `Plan::new` computes them (see `is_automatic`).
    (@items ($name:ident) [$($group:tt)*]) => {
        &[$crate::Token::new(stringify!($name), $crate::__lexer! { @groups $($group)* }, &[])
            .peek_delimiters()]
    };
    (@items ($name:ident) [$($group:tt)*] @ delimited_by ($($delimiter:tt)*)) => {
        &[$crate::Token::new(
            stringify!($name),
            $crate::__lexer! { @groups $($group)* },
            const {
                &[$crate::CharacterGroup {
                    ranges: $crate::__lexer! { @class [] | $($delimiter)* },
                    group_type: $crate::CharacterGroupType::ExactlyOne,
                }]
            },
        )]
    };
    (@items $token:tt $groups:tt $first:literal ..= $last:literal $($rest:tt)*) => {
        $crate::__lexer! {
            @quantifier $token $groups
            (const { &[$crate::dsl::range(concat!($first), concat!($last))] })
            $($rest)*
        }
    };
    (@items $token:tt $groups:tt ($($class:tt)*) $($rest:tt)*) => {
        $crate::__lexer! { @quantifier $token $groups ($crate::__lexer! { @class [] | $($class)* }) $($rest)* }
    };
    (@items $token:tt $groups:tt $fragment:ident $($rest:tt)*) => {
        $crate::__lexer! { @quantifier $token $groups ($fragment) $($rest)* }
    };
    (@items $token:tt $groups:tt $char:literal + $($rest:tt)*) => {
        $crate::__lexer! {
            @quantifier $token $groups
            (const { &[$crate::dsl::range(concat!($char), concat!($char))] })
            + $($rest)*
        }
    };
    (@items $token:tt $groups:tt $char:literal * $($rest:tt)*) => {
        $crate::__lexer! {
            @quantifier $token $groups
            (const { &[$crate::dsl::range(concat!($char), concat!($char))] })
            * $($rest)*
        }
    };
    (@items $token:tt [$($group:tt)*] $string:literal $($rest:tt)*) => {
        $crate::__lexer! { @items $token [$($group)* ($crate::__lexer! { @string $string })] $($rest)* }
    };
    (@items ($name:ident) $groups:tt $($rest:tt)*) => {
        compile_error!(concat!("unexpected `", stringify!($($rest)*), "` in token `", stringify!($name), "`"))
    };

    // `+` and `*` after an item, `ExactlyOne` otherwise.
    (@quantifier $token:tt [$($group:tt)*] $ranges:tt + $($rest:tt)*) => {
        $crate::__lexer! { @items $token [$($group)* ($crate::__lexer! { @group $ranges AtLeastOne })] $($rest)* }
    };
    (@quantifier $token:tt [$($group:tt)*] $ranges:tt * $($rest:tt)*) => {
        $crate::__lexer! { @items $token [$($group)* ($crate::__lexer! { @group $ranges ZeroOrMore })] $($rest)* }
    };
    (@quantifier $token:tt [$($group:tt)*] $ranges:tt $($rest:tt)*) => {
        $crate::__lexer! { @items $token [$($group)* ($crate::__lexer! { @group $ranges ExactlyOne })] $($rest)* }
    };

    (@group $ranges:tt $type:ident) => {
        &[$crate::CharacterGroup { ranges: $ranges, group_type: $crate::CharacterGroupType::$type }]
    };

    (@groups $($group:tt)*) => {
        const {
            &$crate::dsl::groups::<{ $crate::dsl::count_groups(&[$($group),*]) }>(&[$($group),*])
        }
    };

    (@string $string:literal) => {
        const {
            &$crate::dsl::string::<{ $crate::dsl::count_scalars(concat!($string)) }>(const {
                &$crate::dsl::scalars::<{ $crate::dsl::count_scalars(concat!($string)) }>(concat!($string))
            })
        }
    };
}

// Literals reach the DSL through `concat!`, the only way to accept both
// `'c'` and `"string"` in the same position. So every literal is a `&str` here.

// Decodes the UTF-8 scalar starting at `bytes[i]`, returns it with its width.
// `bytes` always comes from a `&str`, so it's valid UTF-8.
const fn decode(bytes: &[u8], i: usize) -> (char, usize) {
    let width = match bytes[i] {
        0x00..=0x7f => 1,
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        _ => 4,
    };
    let mut scalar = if width == 1 {
        bytes[i] as u32
    } else {
        bytes[i] as u32 & (0x7f >> width)
    };
    let mut j = 1;
    while j < width {
        scalar = (scalar << 6) | (bytes[i + j] as u32 & 0x3f);
        j += 1;
    }
    return (char::from_u32(scalar).unwrap(), width);
}

/// Number of Unicode scalars in `string`.
#[doc(hidden)]
pub const fn count_scalars(string: &str) -> usize {
    let bytes = string.as_bytes();
    let mut count = 0;
    let mut i = 0;
    while i < bytes.len() {
        // Continuation bytes are 10xxxxxx.
        if bytes[i] & 0xc0 != 0x80 {
            count += 1;
        }
        i += 1;
    }
    return count;
}

// The only scalar of a `'c'` or `"c"` literal.
const fn scalar(literal: &str) -> char {
    if count_scalars(literal) != 1 {
        panic!(
            "Error: ranges, classes and quantifiers take a single character, e.g. 'a'..='z' or 'a'+."
        );
    }
    return decode(literal.as_bytes(), 0).0;
}

/// `'first'..='last'`, or `'first'` alone when both are the same character.
#[doc(hidden)]
pub const fn range(first: &str, last: &str) -> CharacterRange {
    let (first, last) = (scalar(first), scalar(last));
    if first as u32 == last as u32 {
        return CharacterRange::new().with_start(first).build();
    }
    return CharacterRange::new()
        .with_start(first)
        .with_end(last)
        .build();
}

/// One single character range for every scalar of `string`.
#[doc(hidden)]
pub const fn scalars<const N: usize>(string: &str) -> [CharacterRange; N] {
    let bytes = string.as_bytes();
    let mut out = [const { CharacterRange::new() }; N];
    let mut i = 0;
    let mut k = 0;
    while i < bytes.len() {
        let (scalar, width) = decode(bytes, i);
        out[k] = CharacterRange::new().with_start(scalar).build();
        i += width;
        k += 1;
    }
    return out;
}

/// `"string"` as a sequence of `ExactlyOne` groups, one per character.
#[doc(hidden)]
pub const fn string<const N: usize>(
    ranges: &'static [CharacterRange],
) -> [CharacterGroup<'static>; N] {
    if N == 0 {
        panic!("Error: a string literal can't be empty.");
    }
    let mut out = [const {
        CharacterGroup {
            ranges: &[],
            group_type: CharacterGroupType::ExactlyOne,
        }
    }; N];
    let mut i = 0;
    while i < N {
        out[i].ranges = core::slice::from_ref(&ranges[i]);
        i += 1;
    }
    return out;
}

// `&[&[T]]` -> `[T; N]` concatenation, one for every type the DSL builds.
// `N` is always computed with the matching `count_*` function.

#[doc(hidden)]
pub const fn count_ranges(parts: &[&[CharacterRange]]) -> usize {
    let mut count = 0;
    let mut i = 0;
    while i < parts.len() {
        count += parts[i].len();
        i += 1;
    }
    return count;
}

#[doc(hidden)]
pub const fn ranges<const N: usize>(
    parts: &[&[CharacterRange]],
) -> [CharacterRange; N] {
    let mut out = [const { CharacterRange::new() }; N];
    let mut k = 0;
    let mut i = 0;
    while i < parts.len() {
        let mut j = 0;
        while j < parts[i].len() {
            let range = &parts[i][j];
            out[k] = CharacterRange {
                start: range.start,
                end: range.end,
            };
            k += 1;
            j += 1;
        }
        i += 1;
    }
    return out;
}

#[doc(hidden)]
pub const fn count_groups(parts: &[&[CharacterGroup]]) -> usize {
    let mut count = 0;
    let mut i = 0;
    while i < parts.len() {
        count += parts[i].len();
        i += 1;
    }
    return count;
}

#[doc(hidden)]
pub const fn groups<const N: usize>(
    parts: &[&[CharacterGroup<'static>]],
) -> [CharacterGroup<'static>; N] {
    let mut out = [const {
        CharacterGroup {
            ranges: &[],
            group_type: CharacterGroupType::ExactlyOne,
        }
    }; N];
    let mut k = 0;
    let mut i = 0;
    while i < parts.len() {
        let mut j = 0;
        while j < parts[i].len() {
            let group = &parts[i][j];
            out[k] = CharacterGroup {
                ranges: group.ranges,
                group_type: match group.group_type {
                    CharacterGroupType::ExactlyOne => {
                        CharacterGroupType::ExactlyOne
                    }
                    CharacterGroupType::AtLeastOne => {
                        CharacterGroupType::AtLeastOne
                    }
                    CharacterGroupType::ZeroOrMore => {
                        CharacterGroupType::ZeroOrMore
                    }
                },
            };
            k += 1;
            j += 1;
        }
        i += 1;
    }
    return out;
}

#[doc(hidden)]
pub const fn count_tokens(parts: &[&[Token]]) -> usize {
    let mut count = 0;
    let mut i = 0;
    while i < parts.len() {
        count += parts[i].len();
        i += 1;
    }
    return count;
}

#[doc(hidden)]
pub const fn tokens<const N: usize>(
    parts: &[&[Token<'static>]],
) -> [Token<'static>; N] {
    let mut out = [const { Token::new("", &[], &[]) }; N];
    let mut k = 0;
    let mut i = 0;
    while i < parts.len() {
        let mut j = 0;
        while j < parts[i].len() {
            let token = &parts[i][j];
            out[k] = Token {
                label: token.label,
                groups: token.groups,
                delimiters: token.delimiters,
                consume_delimiters: token.consume_delimiters,
            };
            k += 1;
            j += 1;
        }
        i += 1;
    }
    return out;
}

// Automatic delimiters.
//
// A token written without `@ delimited_by(...)` must end as soon as the next
// character can't continue it, and that character must then be read again as
// the start of the next token. The engine already does the second part with
// peeked delimiters (`Token::peek_delimiters`), so the only job left is to find,
// for every such token, which characters end it: its **follow set**.
//
//     follow set = what may start the next token (any token's first character,
//                  whitespace or the end of input)
//                - what continues some token at that point
//
// "At that point" matters. After `let`, both `LET: "let"` and
// `IDENT: LETTER (LETTER | DIGIT)*` match the input, so a letter must continue
// `IDENT` (`letter`) instead of ending `LET`. What matches the input so far is a
// `Context`: where every token is inside its own pattern.
//
// The building blocks, from the bottom up:
//
// + `CharSet`: a set of characters.
// + `Positions`: where one pattern is after reading some input.
// + `Context`: the positions of every token at once.
// + `explore` / `follow_from`: the follow sets of a token in every context
//   where it is complete.
// + `Plan`: the final tokens, described with indexes.
// + `split`: one copy of a token per follow set, when it depends on what was read.
// + `plan_*`: the plan turned into real `Token`s.

// Checks if `token` was written without `@ delimited_by(...)`, so its delimiters
// must be computed. The DSL marks it with no delimiters and the peek flag,
// a token written by hand like that would be useless since it never ends.
const fn is_automatic(token: &Token) -> bool {
    return token.delimiters.is_empty() && !token.consume_delimiters;
}

// Character sets.
//
// Characters are handled as `u32` scalar values.
const MAX_CHAR: u32 = char::MAX as u32;

// The most ranges a `CharSet` can hold.
const SET_CAPACITY: usize = 64;

// A set of characters, stored as inclusive ranges that are sorted and never
// overlap nor touch (`'a'..='c'` plus `'d'..='f'` is stored as `'a'..='f'`).
// Every operation keeps this shape, so equal sets are always stored the same way.
#[derive(Clone, Copy)]
struct CharSet {
    ranges: [(u32, u32); SET_CAPACITY],
    len: usize,
}

impl CharSet {
    const EMPTY: Self = Self {
        ranges: [(0, 0); SET_CAPACITY],
        len: 0,
    };

    // Returns the characters matched by `group`. Surrogates (`0xD800..=0xDFFF`) are
    // not characters, so they are left out even when a range spans them
    // (`' '..='\u{10FFFF}'`). Every other set is built from these, so no set
    // ever contains a surrogate.
    const fn of_group(group: &CharacterGroup) -> Self {
        let mut set = Self::EMPTY;
        let mut i = 0;
        while i < group.ranges.len() {
            set = match (group.ranges[i].start, group.ranges[i].end) {
                (Some(start), Some(end)) => set.with(start as u32, end as u32),
                (Some(only), None) | (None, Some(only)) => {
                    set.with(only as u32, only as u32)
                }
                (None, None) => set,
            };
            i += 1;
        }
        return set.minus(Self::EMPTY.with(0xd800, 0xdfff));
    }

    const fn is_empty(&self) -> bool {
        return self.len == 0;
    }

    const fn contains(&self, c: u32) -> bool {
        let mut i = 0;
        while i < self.len {
            if self.ranges[i].0 <= c && c <= self.ranges[i].1 {
                return true;
            }
            i += 1;
        }
        return false;
    }

    // Returns the smallest character of a set that is not empty.
    const fn first(&self) -> u32 {
        return self.ranges[0].0;
    }

    const fn equals(&self, other: &Self) -> bool {
        if self.len != other.len {
            return false;
        }
        let mut i = 0;
        while i < self.len {
            if self.ranges[i].0 != other.ranges[i].0
                || self.ranges[i].1 != other.ranges[i].1
            {
                return false;
            }
            i += 1;
        }
        return true;
    }

    // Appends `low..=high`, which must come after every range of the set.
    // Only used by the operations below, which build their result in order.
    const fn push(&mut self, low: u32, high: u32) {
        if self.len == SET_CAPACITY {
            panic!(
                "Error: a character class has too many ranges to compute automatic delimiters."
            );
        }
        self.ranges[self.len] = (low, high);
        self.len += 1;
    }

    // Returns the set plus the characters `low..=high`.
    const fn with(self, low: u32, high: u32) -> Self {
        let mut out = Self::EMPTY;
        let (mut low, mut high) = (low, high);
        let mut added = false;
        let mut i = 0;
        while i < self.len {
            let (start, end) = self.ranges[i];
            if end + 1 < low {
                // Entirely before the new range.
                out.push(start, end);
            } else if high + 1 < start {
                // Entirely after the new range: the new range goes first.
                if !added {
                    out.push(low, high);
                    added = true;
                }
                out.push(start, end);
            } else {
                // Overlapping or touching: the new range absorbs it.
                if start < low {
                    low = start;
                }
                if end > high {
                    high = end;
                }
            }
            i += 1;
        }
        if !added {
            out.push(low, high);
        }
        return out;
    }

    const fn union(self, other: Self) -> Self {
        let mut out = self;
        let mut i = 0;
        while i < other.len {
            out = out.with(other.ranges[i].0, other.ranges[i].1);
            i += 1;
        }
        return out;
    }

    // Returns every character that is not in the set.
    const fn complement(self) -> Self {
        let mut out = Self::EMPTY;
        // The first character not covered by the ranges seen so far.
        let mut next = 0;
        let mut i = 0;
        while i < self.len {
            let (start, end) = self.ranges[i];
            if start > next {
                out.push(next, start - 1);
            }
            next = end + 1;
            i += 1;
        }
        if next <= MAX_CHAR {
            out.push(next, MAX_CHAR);
        }
        return out;
    }

    const fn intersection(self, other: Self) -> Self {
        let mut out = Self::EMPTY;
        let mut i = 0;
        while i < self.len {
            let mut j = 0;
            while j < other.len {
                let low = max(self.ranges[i].0, other.ranges[j].0);
                let high = min(self.ranges[i].1, other.ranges[j].1);
                if low <= high {
                    out = out.with(low, high);
                }
                j += 1;
            }
            i += 1;
        }
        return out;
    }

    // Returns the characters of the set that are not in `other`.
    const fn minus(self, other: Self) -> Self {
        return self.intersection(other.complement());
    }

    const fn overlaps(self, other: Self) -> bool {
        return !self.intersection(other).is_empty();
    }
}

const fn min(a: u32, b: u32) -> u32 {
    return if a < b { a } else { b };
}

const fn max(a: u32, b: u32) -> u32 {
    return if a > b { a } else { b };
}

// Positions.
//
// A pattern is a list of groups, each matching one character once, one or more
// times (`+`) or zero or more times (`*`). Its positions are:
//
// + position `0`: nothing read yet;
// + position `i + 1`: the last character read was matched by group `i`.
//
// Reading `12.5` with `DIGIT+ '.' DIGIT+` goes through positions 0, 1, 1, 2, 3.
//
// The same input can leave a pattern at several positions at once: `'a'* 'a'`
// is both at 1 and 2 after reading `a`. So positions are a set, one bit each.

// A set of positions in a pattern, position `p` is bit `p`.
type Positions = u64;

// Every pattern starts at position 0.
const AT_START: Positions = 1;

// The most groups in a pattern, so that all its positions fit in a `Positions`.
const MAX_GROUPS: usize = 63;

const fn can_repeat(group: &CharacterGroup) -> bool {
    return !matches!(group.group_type, CharacterGroupType::ExactlyOne);
}

const fn can_skip(group: &CharacterGroup) -> bool {
    return matches!(group.group_type, CharacterGroupType::ZeroOrMore);
}

const fn has(positions: Positions, position: usize) -> bool {
    return positions & (1 << position) != 0;
}

// Returns the groups that can match the next character at `position`, one bit each.
const fn next_groups(groups: &[CharacterGroup], position: usize) -> u64 {
    let mut next = 0;
    // The group that matched the last character, if it can repeat.
    if position > 0 && can_repeat(&groups[position - 1]) {
        next |= 1 << (position - 1);
    }
    // The following group, and the ones after it as long as they can be skipped.
    let mut group = position;
    while group < groups.len() {
        next |= 1 << group;
        if !can_skip(&groups[group]) {
            break;
        }
        group += 1;
    }
    return next;
}

// Checks if the pattern is complete at one of `positions`: every group after
// it can be skipped.
const fn is_complete(groups: &[CharacterGroup], positions: Positions) -> bool {
    let mut position = 0;
    while position <= groups.len() {
        if has(positions, position) {
            let mut group = position;
            while group < groups.len() && can_skip(&groups[group]) {
                group += 1;
            }
            if group == groups.len() {
                return true;
            }
        }
        position += 1;
    }
    return false;
}

// Reads `c` at `positions`, returns where the pattern is after it.
// The result is empty if the pattern can't read `c`.
const fn read(
    groups: &[CharacterGroup],
    positions: Positions,
    c: u32,
) -> Positions {
    let mut after = 0;
    let mut position = 0;
    while position <= groups.len() {
        if has(positions, position) {
            let next = next_groups(groups, position);
            let mut group = 0;
            while group < groups.len() {
                if has(next, group)
                    && CharSet::of_group(&groups[group]).contains(c)
                {
                    after |= 1 << (group + 1);
                }
                group += 1;
            }
        }
        position += 1;
    }
    return after;
}

// Returns every character the pattern can read next at `positions`.
const fn next_chars(
    groups: &[CharacterGroup],
    positions: Positions,
) -> CharSet {
    let mut chars = CharSet::EMPTY;
    let mut position = 0;
    while position <= groups.len() {
        if has(positions, position) {
            let next = next_groups(groups, position);
            let mut group = 0;
            while group < groups.len() {
                if has(next, group) {
                    chars = chars.union(CharSet::of_group(&groups[group]));
                }
                group += 1;
            }
        }
        position += 1;
    }
    return chars;
}

// Returns every character the pattern can read, anywhere.
const fn every_char(groups: &[CharacterGroup]) -> CharSet {
    let mut chars = CharSet::EMPTY;
    let mut group = 0;
    while group < groups.len() {
        chars = chars.union(CharSet::of_group(&groups[group]));
        group += 1;
    }
    return chars;
}

// The most tokens in a lexer with automatic delimiters.
const MAX_TOKENS: usize = 128;

// Where every token is after reading the same input: token `t` is at
// `positions[t]`, which is empty once the input stopped matching it.
#[derive(Clone, Copy)]
struct Context {
    positions: [Positions; MAX_TOKENS],
}

impl Context {
    const NONE: Self = Self {
        positions: [0; MAX_TOKENS],
    };

    // Creates the context before reading anything: every token is at its start.
    const fn start(tokens: &[Token]) -> Self {
        let mut context = Self::NONE;
        let mut t = 0;
        while t < tokens.len() {
            context.positions[t] = AT_START;
            t += 1;
        }
        return context;
    }

    // Returns the same context, where only token `t` matches.
    const fn only(&self, t: usize) -> Self {
        let mut context = Self::NONE;
        context.positions[t] = self.positions[t];
        return context;
    }

    // Returns the context after reading `c`.
    const fn read(&self, tokens: &[Token], c: u32) -> Self {
        let mut context = Self::NONE;
        let mut t = 0;
        while t < tokens.len() {
            context.positions[t] = read(tokens[t].groups, self.positions[t], c);
            t += 1;
        }
        return context;
    }

    const fn equals(&self, other: &Self, tokens: &[Token]) -> bool {
        let mut t = 0;
        while t < tokens.len() {
            if self.positions[t] != other.positions[t] {
                return false;
            }
            t += 1;
        }
        return true;
    }

    const fn is_complete(&self, tokens: &[Token], t: usize) -> bool {
        return is_complete(tokens[t].groups, self.positions[t]);
    }

    // Computes the follow set here: what may start the next token, minus every
    // character that continues some token.
    const fn follow(&self, tokens: &[Token], first: CharSet) -> CharSet {
        let mut continuations = CharSet::EMPTY;
        let mut t = 0;
        while t < tokens.len() {
            continuations = continuations
                .union(next_chars(tokens[t].groups, self.positions[t]));
            t += 1;
        }
        return first.minus(continuations);
    }
}

// Returns what may start the next token: the first character of any token,
// the whitespace skipped between tokens, or the end of input (`'\0'`).
const fn first_chars(tokens: &[Token]) -> CharSet {
    let mut first = CharSet::EMPTY
        .with(' ' as u32, ' ' as u32)
        .with('\t' as u32, '\n' as u32)
        .with('\r' as u32, '\r' as u32)
        .with('\0' as u32, '\0' as u32);
    let mut t = 0;
    while t < tokens.len() {
        first = first.union(next_chars(tokens[t].groups, AT_START));
        t += 1;
    }
    return first;
}

// The most atoms the next character of a token can be split into.
const ATOM_CAPACITY: usize = 32;

// The characters a token can read next, split into **atoms**: all the
// characters of an atom move every token to the same positions, so reading
// one of them (the first) tells what reading any of them does.
//
// Next to `HEX: '0' 'x' ...`, the first character of `NUMBER: DIGIT+` is split
// into `'0'` (`HEX` keeps matching) and `'1'..='9'` (it doesn't).
struct Atoms {
    sets: [CharSet; ATOM_CAPACITY],
    len: usize,
}

impl Atoms {
    // Splits the characters token `t` can read next in `context` into atoms.
    const fn of(tokens: &[Token], t: usize, context: &Context) -> Self {
        let mut atoms = Self {
            sets: [CharSet::EMPTY; ATOM_CAPACITY],
            len: 0,
        };
        atoms.add(next_chars(tokens[t].groups, context.positions[t]));
        // Cut them by every group that a token could match next.
        let mut u = 0;
        while u < tokens.len() {
            let groups = tokens[u].groups;
            let mut position = 0;
            while position <= groups.len() {
                if has(context.positions[u], position) {
                    let next = next_groups(groups, position);
                    let mut group = 0;
                    while group < groups.len() {
                        if has(next, group) {
                            atoms =
                                atoms.cut(CharSet::of_group(&groups[group]));
                        }
                        group += 1;
                    }
                }
                position += 1;
            }
            u += 1;
        }
        return atoms;
    }

    const fn add(&mut self, atom: CharSet) {
        if atom.is_empty() {
            return;
        }
        if self.len == ATOM_CAPACITY {
            panic!(
                "Error: too many overlapping tokens to compute automatic delimiters."
            );
        }
        self.sets[self.len] = atom;
        self.len += 1;
    }

    // Splits every atom in two: the characters inside `by` and those outside.
    const fn cut(self, by: CharSet) -> Self {
        let mut out = Self {
            sets: [CharSet::EMPTY; ATOM_CAPACITY],
            len: 0,
        };
        let mut i = 0;
        while i < self.len {
            out.add(self.sets[i].intersection(by));
            out.add(self.sets[i].minus(by));
            i += 1;
        }
        return out;
    }
}

// The follow sets of a token in all the contexts where it is complete.
#[derive(Clone, Copy)]
struct Follow {
    // The characters that end the token in **every** context. Always safe:
    // any other character either continues a token or is an error, so the
    // token is never cut too early.
    chars: CharSet,
    // Whether every context has the same follow set, so `chars` is exact.
    same_everywhere: bool,
    // Whether the token is complete in at least one context.
    found: bool,
}

impl Follow {
    const NOTHING: Self = Self {
        chars: CharSet::EMPTY,
        same_everywhere: true,
        found: false,
    };

    // Adds the follow set of one more context.
    const fn and(self, chars: CharSet) -> Self {
        if !self.found {
            return Self {
                chars: chars,
                same_everywhere: true,
                found: true,
            };
        }
        return Self {
            chars: self.chars.intersection(chars),
            same_everywhere: self.same_everywhere && self.chars.equals(&chars),
            found: true,
        };
    }
}

// The most contexts `explore` can visit.
const EXPLORE_CAPACITY: usize = 64;

// Reads every input that keeps token `t` matching, starting from `from`, and
// collects the follow set of every context where `t` is complete.
//
// Contexts are visited breadth first, each one once. Reading one character
// per atom is enough to reach all of them, since all the characters of an atom
// lead to the same context.
const fn explore(
    tokens: &[Token],
    first: CharSet,
    t: usize,
    from: Context,
) -> Follow {
    let mut seen = [Context::NONE; EXPLORE_CAPACITY];
    seen[0] = from;
    let mut seen_len = 1;
    let mut follow = Follow::NOTHING;
    let mut i = 0;
    while i < seen_len {
        let context = seen[i];
        if context.is_complete(tokens, t) {
            follow = follow.and(context.follow(tokens, first));
        }
        let atoms = Atoms::of(tokens, t, &context);
        let mut a = 0;
        while a < atoms.len {
            let next = context.read(tokens, atoms.sets[a].first());
            let mut known = false;
            let mut j = 0;
            while j < seen_len {
                known = known || seen[j].equals(&next, tokens);
                j += 1;
            }
            if !known {
                if seen_len == EXPLORE_CAPACITY {
                    panic!(
                        "Error: too many overlapping tokens to compute automatic delimiters, give some of them `@ delimited_by(...)`."
                    );
                }
                seen[seen_len] = next;
                seen_len += 1;
            }
            a += 1;
        }
        i += 1;
    }
    return follow;
}

// Computes the follow sets of token `t`, from `context` on (see `explore`).
//
// Exploring with every token can be expensive: next to many keywords, an
// identifier goes through every prefix of every keyword. Most of the time the
// other tokens can't change anything, so this first explores `t` alone, which
// is cheap, and explores with everyone only if another token could matter.
const fn follow_from(
    tokens: &[Token],
    first: CharSet,
    t: usize,
    context: Context,
) -> Follow {
    let alone = explore(tokens, first, t, context.only(t));
    if alone.same_everywhere && !others_matter(tokens, t, &context, alone.chars)
    {
        return alone;
    }
    return explore(tokens, first, t, context);
}

// Checks if another token could continue one of the characters in `ends` at
// some point where `t` is complete, so the follow set of `t` alone is wrong.
const fn others_matter(
    tokens: &[Token],
    t: usize,
    context: &Context,
    ends: CharSet,
) -> bool {
    let next_of_t = next_chars(tokens[t].groups, context.positions[t]);
    let t_is_complete = context.is_complete(tokens, t);
    let mut u = 0;
    while u < tokens.len() {
        if u != t && context.positions[u] != 0 {
            let groups = tokens[u].groups;
            let next_of_u = next_chars(groups, context.positions[u]);
            // Right here, where `t` may already be complete.
            if t_is_complete && next_of_u.overlaps(ends) {
                return true;
            }
            // Later, if `u` can keep matching the same input as `t`. We don't
            // follow `u` further: any of its characters could come next.
            if next_of_u.overlaps(next_of_t)
                && every_char(groups).overlaps(ends)
            {
                return true;
            }
        }
        u += 1;
    }
    return false;
}

// The capacity of a `Plan`.
const PLAN_RANGES: usize = 1024;
const PLAN_GROUPS: usize = 1024;
const PLAN_TOKENS: usize = 256;

// A group of a planned token.
#[derive(Clone, Copy)]
enum PlanGroup {
    // Group `group` of the written token, as it is.
    Written { group: usize },
    // Group `group` of the written token after it matched once: it may match
    // more characters, but doesn't have to (`*`).
    Repeat { group: usize },
    // Exactly one character of `Plan::ranges[start..end]`: an atom read by `split`.
    Atom { start: usize, end: usize },
}

// A planned token.
#[derive(Clone, Copy)]
struct PlanToken {
    // The written token it comes from, for its label and groups.
    token: usize,
    // Its groups: `Plan::groups[start..end]`.
    groups: (usize, usize),
    // Its peeked delimiters, `Plan::ranges[start..end]`, or `None` to keep
    // the delimiters (and the consume flag) of the written token.
    delimiters: Option<(usize, usize)>,
}

/// The final tokens of a lexer, built in a `const fn` from the tokens written
/// in `lexer!`. Everything is an index into the plan or into those tokens; the
/// `plan_*` functions turn it into real `Token`s.
#[doc(hidden)]
pub struct Plan {
    ranges: [(u32, u32); PLAN_RANGES],
    ranges_len: usize,
    groups: [PlanGroup; PLAN_GROUPS],
    groups_len: usize,
    tokens: [PlanToken; PLAN_TOKENS],
    tokens_len: usize,
}

impl Plan {
    /// Plans the final tokens: a token with `@ delimited_by(...)` is kept as
    /// written, the others become one or more copies (see `split`), each with
    /// its follow set as peeked delimiters.
    pub const fn new(tokens: &[Token]) -> Self {
        check_tokens(tokens);
        let first = first_chars(tokens);
        let mut plan = Self {
            ranges: [(0, 0); PLAN_RANGES],
            ranges_len: 0,
            groups: [PlanGroup::Written { group: 0 }; PLAN_GROUPS],
            groups_len: 0,
            tokens: [PlanToken {
                token: 0,
                groups: (0, 0),
                delimiters: None,
            }; PLAN_TOKENS],
            tokens_len: 0,
        };
        let mut t = 0;
        while t < tokens.len() {
            if is_automatic(&tokens[t]) {
                split(
                    &mut plan,
                    tokens,
                    first,
                    t,
                    Context::start(tokens),
                    Prefix::EMPTY,
                );
            } else {
                plan.add_written(tokens, t);
            }
            t += 1;
        }
        return plan;
    }

    /// Number of ranges in the plan.
    pub const fn ranges_len(&self) -> usize {
        return self.ranges_len;
    }

    /// Number of groups in the plan.
    pub const fn groups_len(&self) -> usize {
        return self.groups_len;
    }

    /// Number of planned tokens.
    pub const fn tokens_len(&self) -> usize {
        return self.tokens_len;
    }

    // Adds token `t` as written, delimiters included.
    const fn add_written(&mut self, tokens: &[Token], t: usize) {
        let start = self.groups_len;
        let mut group = 0;
        while group < tokens[t].groups.len() {
            self.add_group(PlanGroup::Written { group: group });
            group += 1;
        }
        self.add_token(t, (start, self.groups_len), None);
    }

    // Adds a copy of token `t` that peeks `follow`. Its groups are the atoms
    // of `prefix`, one character each, then the rest of the written pattern
    // from `position` (`None` for nothing more).
    const fn add_copy(
        &mut self,
        tokens: &[Token],
        t: usize,
        prefix: Prefix,
        position: Option<usize>,
        follow: CharSet,
    ) {
        let start = self.groups_len;
        let mut i = 0;
        while i < prefix.len {
            let (atom_start, atom_end) = prefix.atoms[i];
            self.add_group(PlanGroup::Atom {
                start: atom_start,
                end: atom_end,
            });
            i += 1;
        }
        if let Some(position) = position {
            let groups = tokens[t].groups;
            // The group that matched the last character may match more.
            if position > 0 && can_repeat(&groups[position - 1]) {
                self.add_group(PlanGroup::Repeat {
                    group: position - 1,
                });
            }
            let mut group = position;
            while group < groups.len() {
                self.add_group(PlanGroup::Written { group: group });
                group += 1;
            }
        }
        if follow.is_empty() {
            panic!(
                "Error: a token never ends, every character after it continues a token. Give it `@ delimited_by(...)`."
            );
        }
        let delimiters = self.add_ranges(follow);
        self.add_token(t, (start, self.groups_len), Some(delimiters));
    }

    const fn add_token(
        &mut self,
        t: usize,
        groups: (usize, usize),
        delimiters: Option<(usize, usize)>,
    ) {
        if self.tokens_len == PLAN_TOKENS {
            panic!("Error: too many tokens to compute automatic delimiters.");
        }
        self.tokens[self.tokens_len] = PlanToken {
            token: t,
            groups: groups,
            delimiters: delimiters,
        };
        self.tokens_len += 1;
    }

    const fn add_group(&mut self, group: PlanGroup) {
        if self.groups_len == PLAN_GROUPS {
            panic!("Error: too many groups to compute automatic delimiters.");
        }
        self.groups[self.groups_len] = group;
        self.groups_len += 1;
    }

    // Adds the ranges of `chars`, returns where they are.
    const fn add_ranges(&mut self, chars: CharSet) -> (usize, usize) {
        let start = self.ranges_len;
        let mut i = 0;
        while i < chars.len {
            if self.ranges_len == PLAN_RANGES {
                panic!(
                    "Error: too many ranges to compute automatic delimiters."
                );
            }
            self.ranges[self.ranges_len] = chars.ranges[i];
            self.ranges_len += 1;
            i += 1;
        }
        return (start, self.ranges_len);
    }
}

// Fails compilation on tokens that automatic delimiters can't handle.
const fn check_tokens(tokens: &[Token]) {
    if tokens.len() > MAX_TOKENS {
        panic!("Error: a lexer can have at most 128 tokens.");
    }
    let mut t = 0;
    while t < tokens.len() {
        if tokens[t].groups.len() > MAX_GROUPS {
            panic!("Error: a token can have at most 63 items.");
        }
        if is_automatic(&tokens[t]) && is_complete(tokens[t].groups, AT_START) {
            panic!(
                "Error: a token without `@ delimited_by(...)` must match at least one character."
            );
        }
        t += 1;
    }
}

// Splitting.
//
// A token has one set of delimiters, but its follow set can depend on what
// was read. With `A: 'a'+` next to `AB: "ab"` and `B: 'b'`:
//
// + after `a`, `AB` still matches, so `b` must continue it (`ab` is `AB`);
// + after `aa`, `AB` doesn't, so `b` must end `A` (`aab` is `A`, `B`).
//
// So the token becomes several copies with the same label, each one with its
// own delimiters. `split` reads the token one atom at a time, until its follow
// set stops changing:
//
//     A: 'a'+   becomes   A: 'a'           ends before whitespace or eof
//                         A: 'a' 'a' 'a'*  ends before 'b', whitespace or eof
//
// The engine merges the copies (they share their prefixes) and they have the
// same label, so the output is the one of a single, smarter token.

// How many characters `split` reads at most. If the follow set still changes
// after that, the copy uses the characters that end the token in every
// context: always safe (see `Follow::chars`), but an input that should end
// the token on another character is reported as an error.
const MAX_SPLIT_DEPTH: usize = 4;

// The atoms read so far by `split`, each one stored in `Plan::ranges`.
#[derive(Clone, Copy)]
struct Prefix {
    atoms: [(usize, usize); MAX_SPLIT_DEPTH],
    len: usize,
}

impl Prefix {
    const EMPTY: Self = Self {
        atoms: [(0, 0); MAX_SPLIT_DEPTH],
        len: 0,
    };

    // Returns the prefix followed by one more atom.
    const fn and(self, atom: (usize, usize)) -> Self {
        let mut prefix = self;
        prefix.atoms[prefix.len] = atom;
        prefix.len += 1;
        return prefix;
    }
}

// Plans the copies of token `t` for every input that starts with `prefix`,
// the input that left the tokens in `context`.
const fn split(
    plan: &mut Plan,
    tokens: &[Token],
    first: CharSet,
    t: usize,
    context: Context,
    prefix: Prefix,
) {
    let follow = follow_from(tokens, first, t, context);

    // The follow set doesn't change anymore (or we stop looking): one copy
    // for the rest of the pattern. The token may be at several positions,
    // and each one continues differently, so it's one copy per position.
    if follow.same_everywhere || prefix.len == MAX_SPLIT_DEPTH {
        let mut position = 0;
        while position <= tokens[t].groups.len() {
            if has(context.positions[t], position) {
                plan.add_copy(tokens, t, prefix, Some(position), follow.chars);
            }
            position += 1;
        }
        return;
    }

    // It still changes. If the token is complete here, the input read so far
    // gets its own copy, with the follow set of this context.
    if context.is_complete(tokens, t) {
        plan.add_copy(tokens, t, prefix, None, context.follow(tokens, first));
    }
    // Then read one more atom and look again.
    let atoms = Atoms::of(tokens, t, &context);
    let mut a = 0;
    while a < atoms.len {
        let atom = atoms.sets[a];
        let stored = plan.add_ranges(atom);
        split(
            plan,
            tokens,
            first,
            t,
            context.read(tokens, atom.first()),
            prefix.and(stored),
        );
        a += 1;
    }
}

// Lowering.
//
// The `plan_*` functions turn a `Plan` into real ranges, groups and tokens.
// Each one returns an array of the exact size, read from the plan beforehand.
// **Do not** call them directly, they are used by `lexer!`.

// Returns `items[start..end]`, keeping the `'static` lifetime.
const fn slice<T>(
    items: &'static [T],
    (start, end): (usize, usize),
) -> &'static [T] {
    return items.split_at(start).1.split_at(end - start).0;
}

/// Builds the ranges of the plan: atoms and peeked delimiters.
#[doc(hidden)]
pub const fn plan_ranges<const N: usize>(plan: &Plan) -> [CharacterRange; N] {
    let mut out = [const { CharacterRange::new() }; N];
    let mut i = 0;
    while i < N {
        let (low, high) = plan.ranges[i];
        let low = char::from_u32(low).unwrap();
        let high = char::from_u32(high).unwrap();
        out[i] = if low as u32 == high as u32 {
            CharacterRange::new().with_start(low).build()
        } else {
            CharacterRange::new().with_start(low).with_end(high).build()
        };
        i += 1;
    }
    return out;
}

/// Builds the groups of every planned token.
#[doc(hidden)]
pub const fn plan_groups<const N: usize>(
    plan: &Plan,
    tokens: &'static [Token<'static>],
    ranges: &'static [CharacterRange],
) -> [CharacterGroup<'static>; N] {
    let mut out = [const {
        CharacterGroup {
            ranges: &[],
            group_type: CharacterGroupType::ExactlyOne,
        }
    }; N];
    let mut p = 0;
    while p < plan.tokens_len {
        let token = &tokens[plan.tokens[p].token];
        let (start, end) = plan.tokens[p].groups;
        let mut i = start;
        while i < end {
            out[i] = match plan.groups[i] {
                PlanGroup::Written { group } => {
                    copy_group(&token.groups[group])
                }
                PlanGroup::Repeat { group } => CharacterGroup {
                    ranges: token.groups[group].ranges,
                    group_type: CharacterGroupType::ZeroOrMore,
                },
                PlanGroup::Atom { start, end } => CharacterGroup {
                    ranges: slice(ranges, (start, end)),
                    group_type: CharacterGroupType::ExactlyOne,
                },
            };
            i += 1;
        }
        p += 1;
    }
    return out;
}

const fn copy_group(
    group: &CharacterGroup<'static>,
) -> CharacterGroup<'static> {
    return CharacterGroup {
        ranges: group.ranges,
        group_type: match group.group_type {
            CharacterGroupType::ExactlyOne => CharacterGroupType::ExactlyOne,
            CharacterGroupType::AtLeastOne => CharacterGroupType::AtLeastOne,
            CharacterGroupType::ZeroOrMore => CharacterGroupType::ZeroOrMore,
        },
    };
}

/// Builds one delimiter group per planned token (unused for written delimiters).
#[doc(hidden)]
pub const fn plan_delimiters<const N: usize>(
    plan: &Plan,
    ranges: &'static [CharacterRange],
) -> [CharacterGroup<'static>; N] {
    let mut out = [const {
        CharacterGroup {
            ranges: &[],
            group_type: CharacterGroupType::ExactlyOne,
        }
    }; N];
    let mut p = 0;
    while p < N {
        if let Some(span) = plan.tokens[p].delimiters {
            out[p].ranges = slice(ranges, span);
        }
        p += 1;
    }
    return out;
}

/// Builds the planned tokens.
#[doc(hidden)]
pub const fn plan_tokens<const N: usize>(
    plan: &Plan,
    tokens: &'static [Token<'static>],
    groups: &'static [CharacterGroup<'static>],
    delimiters: &'static [CharacterGroup<'static>],
) -> [Token<'static>; N] {
    let mut out = [const { Token::new("", &[], &[]) }; N];
    let mut p = 0;
    while p < N {
        let planned = plan.tokens[p];
        let token = &tokens[planned.token];
        out[p] = match planned.delimiters {
            Some(_) => Token {
                label: token.label,
                groups: slice(groups, planned.groups),
                delimiters: core::slice::from_ref(&delimiters[p]),
                consume_delimiters: false,
            },
            None => Token {
                label: token.label,
                groups: slice(groups, planned.groups),
                delimiters: token.delimiters,
                consume_delimiters: token.consume_delimiters,
            },
        };
        p += 1;
    }
    return out;
}
