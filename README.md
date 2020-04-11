# Another BitTorrent client written in Rust

This is a simple cli program that will download torrents.  The basic command is

```sh
ntorrent file.torrent
```

There are options for specifying the upload port number and download directory.  See `ntorrent --help` for details.  `ntorrent` stores the download in memory and only writes to file upon completion of download.  As such `ntorrent` cannot resume partial downloads, but it will confirm overwriting of any files that are already present.  However, `ntorrent` can seed files that have been completely downloaded.
