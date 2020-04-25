# Another BitTorrent client written in Rust

This is a simple cli program that will download torrents.  The basic command is

```sh
ntorrent file.torrent
```

There are options for specifying the upload port number and download directory.  See `ntorrent --help` for details.  `ntorrent` stores the download in memory and writes to file upon completion of download.  Partial downloads get recorded to a .part file which allows `ntorrent` to resume downloads.  Completely downloaded files can also be seeded.
