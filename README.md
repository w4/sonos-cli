# sonos-cli

Control and monitor your Sonos speakers from the command line.

## Build from source

```bash
$ git clone git@github.com:w4/sonos-cli.git && cd sonos-cli
$ cargo build --release
$ mv target/release/sonos-cli /usr/local/bin/sonos
```

## Usage

```
$ sonos help
sonos 0.2.0
Jordan Doyle <jordan@doyle.la>
Control your Sonos using the command line

USAGE:
    sonos -c <IP or Room Name> [json] <SUBCOMMAND>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -c <IP or Room Name>        Set the controller to run operation on

ARGS:
    <json>    Return back JSON serialised responses for programmatic use of the CLI

SUBCOMMANDS:
    help      Prints this message or the help of the given subcommand(s)
    info      Shows information about the speaker
    rooms     List all of your speakers
    seek      Seek to a specific timestamp on the current track
    track     Commands to manipulate the tracklist
    volume    Get or set the volume of the speaker

$ sonos track help
sonos-track
Commands to manipulate the tracklist

USAGE:
    sonos -c <IP or Room Name> track [SUBCOMMAND]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

SUBCOMMANDS:
    help    Prints this message or the help of the given subcommand(s)
    list    Get the list of tracks in the queue
    next    Skip to the next track
    play    Play a given track
    prev    Go back to the last track

$ sonos track play help
sonos-track-play
Play a given track

USAGE:
    sonos track play [uri] [SUBCOMMAND]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

ARGS:
    <uri>    Queue position to skip to or a Sonos URI to play

SUBCOMMANDS:
    help       Prints this message or the help of the given subcommand(s)
    line-in    Set the current speaker's input to the line-in
    tv         Set the current speaker's input to the SPDIF
```

```
$ sonos -c "Kitchen" track list
1: Afghan Dan & BGM - Resurrection Business (Freestyle) (01:53)
2: The Notorious B.I.G. - Nasty Girl (feat. Diddy, Nelly, Jagged Edge & Avery Storm) [2007 Remaster] (04:46)
3: The Strokes - Juicebox (03:17)
...

$ sonos -c "Kitchen" track play 2

$ sonos -c "Kitchen" seek 02:30

$ sonos -c "Kitchen" track
🎤  The Notorious B.I.G.
🎵  Nasty Girl (feat. Diddy, Nelly, Jagged Edge & Avery Storm) [2007 Remaster]
💿  Greatest Hits
⏱️  02:31/04:46 [▇▇▇▇▇▇▇▇▇▇▇▇▇------------]

$ sonos -c "Kitchen" track next

$ sonos -c "Kitchen" track
🎤  The Strokes
🎵  Juicebox
💿  First Impressions Of Earth
⏱️  00:01/03:17 [-------------------------]
```
