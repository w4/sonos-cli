#[macro_use] extern crate clap;
#[macro_use] extern crate log;
#[macro_use] extern crate serde_derive;
#[macro_use] extern crate failure;

#[macro_use] mod util;
mod discovery;

use std::time::Duration;
use std::net::IpAddr;

use sonos::Speaker;

use failure::Fallible;

fn argparse<'a, 'b>() -> clap::App<'a, 'b> {
    use clap::{App, AppSettings, Arg, SubCommand};

    App::new("sonos")
        .version(crate_version!())
        .author("Jordan Doyle <jordan@doyle.la>")
        .about("Control your Sonos using the command line")
        .setting(AppSettings::SubcommandRequired)
        .arg(Arg::with_name("controller")
                .help("Set the controller to run operation on")
                .short("c")
                .required_unless("rooms")
                .value_name("IP or Room Name")
                .takes_value(true))
        .arg(Arg::with_name("json")
                .help("Return back JSON serialised responses for programmatic use of the CLI"))
        .subcommand(SubCommand::with_name("info").about("Shows information about the speaker"))
        .subcommand(
            SubCommand::with_name("track")
                .about("Commands to manipulate the tracklist")
                .subcommand(SubCommand::with_name("next").about("Skip to the next track"))
                .subcommand(SubCommand::with_name("prev").about("Go back to the last track"))
                .subcommand(SubCommand::with_name("list").about("Get the list of tracks in the queue"))
                .subcommand(
                    SubCommand::with_name("play")
                        .about("Play a given track")
                        .subcommand(SubCommand::with_name("tv").about("Set the current speaker's input to the SPDIF"))
                        .subcommand(SubCommand::with_name("line-in").about("Set the current speaker's input to the line-in"))
                        .arg(Arg::with_name("uri").help("Queue position to skip to or a Sonos URI to play").index(1).conflicts_with_all(&["tv", "line-in"]))
                )
        )
        .subcommand(SubCommand::with_name("seek").about("Seek to a specific timestamp on the current track")
                        .arg(Arg::with_name("TIMESTAMP")
                                .help("hh:mm:ss/mm:ss")
                                .required(true)
                                .index(1)))
        .subcommand(SubCommand::with_name("volume").about("Get or set the volume of the speaker")
                        .arg(Arg::with_name("VOLUME")
                                .help("Percent volume to set speaker to 0-100")
                                .index(1)))
        .subcommand(SubCommand::with_name("rooms").about("List all of your speakers")
                        .arg(Arg::with_name("invalidate").help("Detect new speakers and room arrangements")))
}

#[tokio::main]
async fn main() -> Fallible<()> {
    let args = argparse().get_matches();

    util::setup_logger()?;

    let controller = args.value_of("controller").expect("controller");
    let speaker = match controller.parse::<IpAddr>() {
        Ok(ip) => Speaker::from_ip(ip).await?,
        Err(_) => discovery::find_speaker_by_name(controller).await?,
    };

    match args.subcommand() {
        ("track", Some(subargs)) => {
            match subargs.subcommand() {
                ("next", _) => speaker.queue().next().await?,
                ("prev", _) => speaker.queue().previous().await?,
                ("list", _) => print_struct!(args, &TrackList::new(&speaker).await?),
                ("play", Some(play_subargs)) => match play_subargs.subcommand_name() {
                    Some("tv") => speaker.play_tv().await?,
                    Some("line-in") => speaker.play_line_in().await?,
                    _ => {
                        let uri = play_subargs.value_of("uri")
                            .filter(|s| !s.is_empty())
                            .ok_or_else(|| format_err!("Must pass [tv], [line-in] or a URI to the play command"))?;

                        if let Ok(pos) = uri.parse::<u64>() {
                            speaker.queue().skip_to(&pos).await?
                        } else {
                            speaker.play_track(uri).await?
                        }
                    },
                },
                _ => print_struct!(args, &Track::new(&speaker).await?)
            }
        },
        ("info", _) => print_struct!(args, &Info::new(&speaker)),
        ("volume", Some(sub)) => match sub.value_of("VOLUME") {
            Some(volume) => speaker.set_volume(volume.parse()?).await?,
            None => print_struct!(args, &Volume::new(&speaker).await?),
        },
        ("seek", Some(sub)) => {
            let a = sub.value_of("TIMESTAMP").expect("timestamp");

            let mut multiplier = 1;

            let secs = a.split(":").collect::<Vec<&str>>().iter().rfold(0, |curr, iter_val| {
                let section_value = iter_val.parse::<u64>().expect("can't parse int") * multiplier;
                multiplier *= 60;
                curr + section_value
            });

            let duration = Duration::new(secs, 0);

            speaker.seek(&duration).await?;
        },
        ("rooms", Some(sub)) => {
            let devices = discovery::discover(true, sub.is_present("invalidate")).await?;

            let mut rooms = std::collections::HashMap::new();

            for device in devices {
                let coordinator = device.coordinator().await?;

                let mut room = rooms.entry(coordinator).or_insert(Vec::new());
                room.push(device);
            }

            for (key, value) in rooms {
                info!("Controller: {}", key);

                for device in value {
                    info!("d:     {}", device.name);
                }
            }
        },
        _ => {
            panic!();
        }
    }

    Ok(())
}

#[derive(Serialize, Deserialize, Debug)]
struct TrackListItem {
    pub position: u64,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: Duration
}
#[derive(Serialize, Deserialize, Debug)]
struct TrackList(Vec<TrackListItem>);
impl TrackList {
    pub async fn new(speaker: &Speaker) -> Fallible<Self> {
        Ok(Self(
            speaker.queue().list().await?
                .into_iter()
                .map(|v| TrackListItem {
                    position: v.position,
                    title: v.title,
                    artist: v.artist,
                    album: v.album,
                    duration: v.duration
                })
                .collect()
        ))
    }
}
impl std::fmt::Display for TrackList {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        Ok(for item in &self.0 {
            writeln!(f, "{}: {} - {} ({})",
                   item.position,
                   item.artist,
                   item.title,
                   util::duration_to_hms(item.duration))?
        })
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct Track {
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub running_time: Duration,
    pub duration: Duration
}
impl Track {
    pub async fn new(speaker: &Speaker) -> Fallible<Track> {
        let track = speaker.track().await?;

        Ok(Self {
            title: track.title,
            artist: track.artist,
            album: track.album,
            running_time: track.running_time,
            duration: track.duration
        })
    }
}
impl std::fmt::Display for Track {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        writeln!(f, "\u{1F3A4}  {}", self.artist)?;
        writeln!(f, "\u{1F3B5}  {}", self.title)?;

        if let Some(album) = &self.album {
            writeln!(f, "\u{1F4BF}  {}", album)?;
        }

        let running_time = util::duration_to_hms(self.running_time);
        let duration = util::duration_to_hms(self.duration);

        write!(f, "\u{23F1}\u{FE0F}  {}/{}", running_time, duration)?;

        const PROG_BAR_LEN: usize = 25;
        let percent_played = ((self.running_time.as_secs() as f64 / self.duration.as_secs() as f64) * PROG_BAR_LEN as f64) as usize;
        write!(f, " [{}{}]", "\u{2587}".repeat(percent_played), "-".repeat(PROG_BAR_LEN - percent_played))
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct Volume {
    volume: u8,
    muted: bool,
}
impl Volume {
    pub async fn new(speaker: &Speaker) -> Result<Volume, failure::Error> {
        Ok(Self {
            volume: speaker.volume().await?,
            muted: speaker.muted().await?,
        })
    }
}
impl std::fmt::Display for Volume {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        const MAX_VOLUME: usize = 100;
        const PROG_BAR_LEN: usize = 25;

        let pictogram = if self.muted {
            "\u{1F507}"
        } else {
            "\u{1F50A}"
        };

        write!(f, "{} {}/{}", pictogram, self.volume, MAX_VOLUME)?;

        let percent = (self.volume as usize * PROG_BAR_LEN) / MAX_VOLUME;

        write!(f, " [{}{}]", "\u{2587}".repeat(percent), "-".repeat(PROG_BAR_LEN - percent))
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct Info {
    pub ip: IpAddr,
    pub model: String,
    pub model_number: String,
    pub software_version: String,
    pub hardware_version: String,
    pub serial_number: String,
    pub name: String,
    pub uuid: String,
}
impl Info {
    pub fn new(speaker: &Speaker) -> Info {
        Info {
            ip: speaker.ip.clone(),
            model: speaker.model.clone(),
            model_number: speaker.model_number.clone(),
            software_version: speaker.software_version.clone(),
            hardware_version: speaker.hardware_version.clone(),
            serial_number: speaker.serial_number.clone(),
            name: speaker.name.clone(),
            uuid: speaker.uuid.clone(),
        }
    }
}
impl std::fmt::Display for Info {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        writeln!(f, "\u{1F508}  {}", self.name)?;
        writeln!(f, "{}", "=".repeat(self.name.len() + 3))?;

        writeln!(f, "Model: {} ({})", self.model, self.model_number)?;

        writeln!(f, "Versions: Software {}, Hardware {}", self.software_version, self.hardware_version)?;
        writeln!(f, "Serial number: {}", self.serial_number)?;
        writeln!(f, "UUID: {}", self.uuid)
    }
}
