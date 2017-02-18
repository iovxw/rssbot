#[macro_use]
extern crate error_chain;
extern crate quick_xml;
extern crate rss;
extern crate atom_syndication as atom;

use std::io::prelude::*;
use std::fs::File;

mod errors;
mod feed;

fn main() {
    let mut file = File::open("example.xml").unwrap();
    let mut s = String::new();
    file.read_to_string(&mut s).unwrap();

    let channel = feed::parse(&s).unwrap();
    println!("{}", channel.title);
    for item in channel.items {
        println!("{}", item.title.unwrap());
    }
}
