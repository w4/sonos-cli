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
sonos 0.1.0
Jordan Doyle <jordan@doyle.la>
Control your Sonos using the command line

USAGE:
    sonos-cli [OPTIONS] [json] <SUBCOMMAND>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -c <IP or Room Name>        Set the controller to run operation on

ARGS:
    <json>    Return back JSON serialised responses for programmatic use of the CLI

SUBCOMMANDS:
    help        Prints this message or the help of the given subcommand(s)
    info        Shows information about the speaker
    next        Skip to the next track
    previous    Go back to the last track
    rooms       List all of your speakers
    seek        Seek to a specific timestamp on the current track
    track       Show the current track information
    volume      Get or set the volume of the speaker
```
