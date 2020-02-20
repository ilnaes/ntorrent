use std::env;
use std::process;

mod torrent_file;

pub use crate::torrent_file::torrent;

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        println!("Need to input exactly one file.");
        process::exit(0);
    }

    let mut t = torrent::new(&args[1]).unwrap_or_else(|err| {
        println!("{}", err);
        process::exit(0);
    });

    t.download().await;
}
