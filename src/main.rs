use std::env;
use std::process;

mod torrent;

pub use crate::torrent::torrent_file;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        println!("Need to input exactly one file.");
        process::exit(0);
    }

    let filename = &args[1];

    let point = torrent_file::TorrentFile { x: 1, y: 2 };

    // Convert the Point to a JSON string.
    let serialized = serde_bencode::to_string(&point).unwrap();

    // Prints serialized = {"x":1,"y":2}
    println!("serialized = {}", serialized);

    // Convert the JSON string back to a Point.
    let deserialized: torrent_file::TorrentFile = serde_bencode::from_str(&serialized).unwrap();

    // Prints deserialized = Point { x: 1, y: 2 }
    println!("deserialized = {:?}", deserialized);
}
