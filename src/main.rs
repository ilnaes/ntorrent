use std::env;
use std::process;
use crate::client::Client;

mod opstream;
mod torrents;
mod client;
mod messages;
mod peerlist;
mod utils;
mod worker;
mod consts;

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        println!("Need to input exactly one file.");
        process::exit(0);
    }

    let mut t = Client::new(&args[1]).await;

    t.download().await;
}
