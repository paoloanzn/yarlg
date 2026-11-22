// The `lexer!` DSL.
//
// `lexer!` is syntactic sugar over [`Token`], [`CharacterGroup`] and `table_init!`:
// every rule is lowered at compile time into the same structures you would
// write by hand, so the resulting table is identical.
//
// The rules are parsed by `__lexer!` in two passes:
//
// + `@split` cuts the input at every `;` (up to 16 tokens per step, which keeps
//   the macro recursion depth close to the number of rules).
// + Every rule is then expanded side by side: fragments become `const` ranges,
//   tokens are collected **in order** into `__TOKENS`.

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
/// + A token without `@ delimited_by(...)` ends at `' '`, `'\t'`, `'\n'`, `'\r'`
///   or `eof`. The delimiter is consumed and is not part of the token text.
/// + When two tokens match the same input the **last declared** one wins.
///
/// # Example
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
        pub(super) const __TOKENS: &[$crate::Token<'static>] =
            $crate::__lexer! { @tokens $( ($crate::__lexer! { @token $r }) )* };
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
    (@items ($name:ident) [$($group:tt)*]) => {
        &[$crate::Token::new(
            stringify!($name),
            $crate::__lexer! { @groups $($group)* },
            $crate::dsl::DEFAULT_DELIMITERS,
        )]
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

/// Delimiters of every token without `@ delimited_by(...)`: whitespace and the end of input.
#[doc(hidden)]
pub const DEFAULT_DELIMITERS: &[CharacterGroup<'static>] = &[CharacterGroup {
    ranges: &[
        CharacterRange::new().with_start(' ').build(),
        CharacterRange::new().with_start('\t').build(),
        CharacterRange::new().with_start('\n').build(),
        CharacterRange::new().with_start('\r').build(),
        CharacterRange::new().with_start('\0').build(),
    ],
    group_type: CharacterGroupType::ExactlyOne,
}];

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
