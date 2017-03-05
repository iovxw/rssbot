//! A Telegram file which contains a readable source and a filename
//!
//! The filename should be such that it represents the content type.

use std::io::Read;
use std::fs;

/// A Telegram file which contains a readable source and a filename
pub struct File {
    pub name: String,
    pub source: Box<Read>,
}

/// Construct a Telegram file from a local path
impl<'a> From<&'a str> for File {
    fn from(path: &'a str) -> File {
        let file = fs::File::open(path).unwrap();

        File {
            name: path.into(),
            source: Box::new(file),
        }
    }
}

/// Construct a Telegram file from an object which implements the Read trait
impl<'a, S: Read + 'static> From<(&'a str, S)> for File {
    fn from((path, source): (&'a str, S)) -> File
        where S: Read + 'static
    {
        File {
            name: path.into(),
            source: Box::new(source),
        }
    }
}
