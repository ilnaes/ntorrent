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
        t.download().await;
    } else {
        t.upload().await;
    }
}
