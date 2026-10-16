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

pub fn main() {
    let mut buf: [u8; 4096] = [0; 4096];
    let bytes_read = read_file_as_bytes("./test/numbers.yrl", &mut buf);
    let readable_buf = match str::from_utf8(&buf) {
        Ok(readable_buf) => readable_buf,
        Err(error) => {
            eprint!("Error: Error convering bytes to string: {}", error);
            panic!("An unexpected error occurred.")
        }
    };
    println!("Bytes read: {} Bytes\nFile:\n{}", bytes_read, readable_buf);

    let digit = match yarlg::AsciiCharacterRange::new()
        .with_start('0' as u8).with_end('9' as u8)
        .build()
    {
        Ok(range) => range,
        Err(error) => {
            eprint!("Error: Error creating AsciiCharacterRange: {}", error);
            panic!("An unexpected error occurred.")
        }
    };
    let group = yarlg::group!(digit, yarlg::CharacterGroupType::AtLeastOne);
    println!("NUMBER: {}", group);
}
