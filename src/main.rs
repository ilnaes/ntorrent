use crate::client::Client;
use crate::torrents::Torrent;
use clap::{App, Arg};

mod client;
mod consts;
mod messages;
mod opstream;
mod partial;
mod peerlist;
mod torrents;
mod utils;
mod worker;

#[tokio::main]
async fn main() {
    let matches = App::new("ntorrent")
        .arg(
            Arg::with_name("INPUT")
                .required(true)
                .help("The .torrent file you want to use")
                .index(1),
        )
        .arg(
            Arg::with_name("p")
                .short("p")
                .help("The port you want to listen on (default: 4444)")
                .value_name("PORT"),
        )
        .arg(
            Arg::with_name("d")
                .short("d")
                .help("The director you want to download to (default: current directory)")
                .value_name("PORT"),
        )
        .get_matches();

    let file = matches.value_of("INPUT").unwrap();
    let port: u16 = matches.value_of("p").unwrap_or("4444").parse().unwrap();
    let dir = matches.value_of("d").unwrap_or("");

    let torrent = Torrent::new(file, dir);
    let mut t = Client::from(&torrent, port).await;
    let res = t.partial.has().await;
    if res == None {
        t.serve(true).await;
    } else if let Some(true) = res {
        println!("SEEDING");
        t.serve(false).await;
    }

    // if res == None {
    //     t.serve(true).await;
    // } else if res == Some(true) {
    //     println!("SEEDING");
    //     t.serve(false).await;
    // } else {
    //     let mut done = false;
    //     while !done {
    //         println!("Will overwrite some files.  Continue? (y/n)");
    //         let mut input = String::new();

    //         if let Ok(_) = std::io::stdin().read_line(&mut input) {
    //             let s = input.to_ascii_lowercase();
    //             let s = s.trim();

    //             if s == "y" || s == "yes" {
    //                 t.serve(true).await;
    //                 done = true;
    //             } else if s == "n" || s == "no" {
    //                 done = true;
    //             }
    //         }
    //     }
    // }
}
