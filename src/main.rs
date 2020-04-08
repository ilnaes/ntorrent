use crate::client::Client;
use clap::{Arg, App};

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
    let matches = App::new("ntorrent")
                    .arg(Arg::with_name("INPUT")
                            .required(true)
                            .help("The .torrent file you want to use")
                            .index(1))
                    .arg(Arg::with_name("p")
                            .short("p")
                            .help("The port you want to listen on (default: 4444)")
                            .value_name("PORT"))
                    .get_matches();

    let file = matches.value_of("INPUT").unwrap();
    let port: u16 = matches.value_of("p").unwrap_or("4444").parse().unwrap();

    let mut t = Client::new(file, String::new(), port).await;

    let res = t.has().await;

    if res == None {
        t.serve(true).await;
    } else if res == Some(true) {
        println!("SEEDING");
        t.serve(false).await;
    } else {
        let mut done = false;
        while !done {
            println!("Will overwrite some files.  Continue? (y/n)");
            let mut input = String::new();

            if let Ok(_) = std::io::stdin().read_line(&mut input) {
                let s = input.to_ascii_lowercase();
                let s = s.trim();

                if s == "y" || s == "yes" {
                    t.serve(true).await;
                    done = true;
                } else if s == "n" || s == "no" {
                    done = true;
                }
            }
        }
    }
}
