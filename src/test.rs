// test.rs

use std::{
    fs::File,
    io::{self, Read},
};

use yarlg;

fn read_file_as_bytes(file_path: &str, read_buf: &mut [u8]) -> usize {
    let mut fp = match File::open(file_path) {
        Ok(fp) => fp,
        Err(error) => match error.kind() {
            io::ErrorKind::NotFound => {
                eprintln!("Error: File not found: {}", error);
                panic!("File not found.");
            }
            _ => {
                eprintln!("Error: Error opening file: {}", error);
                panic!("An unexpected I/O error occurred.");
            }
        },
    };

    let bytes_read: usize = match fp.read(read_buf) {
        Ok(n) => n,
        Err(error) => {
            eprintln!("Error: Error reading file: {}", error);
            0
        }
    };

    bytes_read
}

// A simple testing lexer:
// NUMBER {'0'...'9'}+ ' '
// IDENT  {'a'...'z'}+ ' '
const DIGIT: yarlg::AsciiCharacterRange = yarlg::AsciiCharacterRange::new()
    .with_start('0' as u8)
    .with_end('9' as u8)
    .build();
const LOWER: yarlg::AsciiCharacterRange = yarlg::AsciiCharacterRange::new()
    .with_start('a' as u8)
    .with_end('z' as u8)
    .build();
const SPACE: yarlg::AsciiCharacterRange = yarlg::AsciiCharacterRange::new()
    .with_start(' ' as u8)
    .build();
const NULL: yarlg::AsciiCharacterRange = yarlg::AsciiCharacterRange::new()
    .with_start('\0' as u8)
    .build();

const TOKENS: &[yarlg::Token] = &[
    yarlg::Token::new(
        "NUMBER",
        &[yarlg::group!(DIGIT, yarlg::CharacterGroupType::AtLeastOne)],
        &[
            yarlg::group!(SPACE, yarlg::CharacterGroupType::ExactlyOne),
            yarlg::group!(NULL, yarlg::CharacterGroupType::ExactlyOne),
        ],
    ),
    yarlg::Token::new(
        "IDENT",
        &[yarlg::group!(LOWER, yarlg::CharacterGroupType::AtLeastOne)],
        &[
            yarlg::group!(SPACE, yarlg::CharacterGroupType::ExactlyOne),
            yarlg::group!(NULL, yarlg::CharacterGroupType::ExactlyOne),
        ],
    ),
];

yarlg::table_init!(LEXER, TOKENS);

pub fn main() {
    let mut buf: [u8; 4096] = [0; 4096];
    let bytes_read = read_file_as_bytes("./test/numbers.yrl", &mut buf);
    let readable_buf = match str::from_utf8(&buf[..bytes_read]) {
        Ok(readable_buf) => readable_buf,
        Err(error) => {
            eprint!("Error: Error convering bytes to string: {}", error);
            panic!("An unexpected error occurred.")
        }
    };
    println!("Bytes read: {} Bytes\nFile:\n{}", bytes_read, readable_buf);

    let digit = yarlg::AsciiCharacterRange::new()
        .with_start('0' as u8)
        .with_end('9' as u8)
        .build();
    let group: yarlg::CharacterGroup<'_> =
        yarlg::group!(digit, yarlg::CharacterGroupType::AtLeastOne);
    println!("NUMBER: {}", group);

    let input = "42 foo 1337 bar 7";
    println!("Lexing: {:?}", input);
    match yarlg::Lexer::run(input, &LEXER) {
        Ok(tokens) => println!("Tokens: {:?}", tokens),
        Err(error) => eprintln!("{}", error),
    }
    match yarlg::Lexer::run("42 Foo ", &LEXER) {
        Ok(tokens) => println!("Tokens: {:?}", tokens),
        Err(error) => eprintln!("{}", error),
    }
}
