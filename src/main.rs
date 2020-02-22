#![feature(iter_rfold)]
#[macro_use]
extern crate clap;

extern crate sonos;

#[macro_use]
extern crate log;
extern crate fern;

#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_json;

extern crate strsim;

use std::net::IpAddr;
use sonos::Speaker;

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
        .subcommand(SubCommand::with_name("track").about("Show the current track information"))
        .subcommand(SubCommand::with_name("next").about("Skip to the next track"))
        .subcommand(SubCommand::with_name("previous").about("Go back to the last track"))
        .subcommand(SubCommand::with_name("info").about("Shows information about the speaker"))
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

fn setup_logger() -> Result<(), fern::InitError> {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{}",
                message
            ))
        })
        .level(log::LevelFilter::Info)
        .chain(std::io::stdout())
        .apply()?;
    Ok(())
}

fn duration_to_hms(d: std::time::Duration) -> String {
    let mut s = String::new();

    const SECS_IN_MIN: u64 = 60;
    const MINS_IN_HOUR: u64 = 60;
    const SECS_IN_HOUR: u64 = SECS_IN_MIN * MINS_IN_HOUR;

    let hours = d.as_secs() / SECS_IN_HOUR;
    if hours > 0 {
        s.push_str(&format!("{:02}:", hours));
    }

    s.push_str(&format!("{:02}:", d.as_secs() % SECS_IN_HOUR / SECS_IN_MIN));
    s.push_str(&format!("{:02}", d.as_secs() % SECS_IN_MIN));

    s
}

fn main() {
    let args = argparse().get_matches();

    setup_logger().expect("logger");

    let controller = args.value_of("controller").expect("controller");
    let speaker = if let Ok(ip) = controller.parse::<IpAddr>() {
        Speaker::from_ip(ip).expect("speaker")
    } else {
        let mut speakers = discover(true, false);

        let mut min = 100;

        speakers.sort_by(|a, b| {
            let a = strsim::damerau_levenshtein(&a.name, controller);
            let b = strsim::damerau_levenshtein(&b.name, controller);

            if a < min { min = a; }
            if b < min { min = b; }

            a.cmp(&b)
        });

        if min > 5 {
            panic!("Couldn't find a speaker by that name");
        }

        let speaker = speakers.remove(0);

        if min > 2 {
            use std::io::{Read, Write};
            print!("Couldn't find speaker '{}', did you mean {}? [Y/n] ", controller, speaker.name);
            std::io::stdout().flush();

            let input: char = std::io::stdin()
                .bytes()
                .next()
                .and_then(|result| result.ok())
                .map(|byte| byte as char)
                .unwrap();

            if input != 'y' && input != 'Y' {
                panic!();
            }
        }

        speaker
    };

    match args.subcommand() {
        ("track", _) => {
            let t = Track::new(&speaker).expect("track");
            info!("{}", if args.is_present("json") {
                serde_json::to_string(&t).expect("serialise track")
            } else {
                t.to_string()
            })
        },
        ("next", _) => speaker.next().expect("next"),
        ("previous", _) => speaker.previous().expect("prev"),
        ("info", _) => {
            let i = Info::new(&speaker);
            info!("{}", if args.is_present("json") {
                serde_json::to_string(&i).expect("serialise info")
            } else {
                i.to_string()
            })
        },
        ("volume", Some(sub)) => {
            if let Some(volume) = sub.value_of("VOLUME") {
                speaker.set_volume(volume.parse().unwrap());
            } else {
                info!("{}", Volume::new(speaker.volume().unwrap(), speaker.muted().unwrap()));
            }
        },
        ("seek", Some(sub)) => {
            let a = sub.value_of("TIMESTAMP").expect("timestamp");

            let mut multiplier = 1;

            let secs = a.split(":").collect::<Vec<&str>>().iter().rfold(0, |curr, iter_val| {
                let section_value = iter_val.parse::<u64>().expect("Can't parse int") * multiplier;
                multiplier *= 60;
                curr + section_value
            });

            let duration = std::time::Duration::new(secs, 0);

            speaker.seek(&duration).expect("couldn't seek");
        },
        ("rooms", Some(sub)) => {
            let devices = discover(true, sub.is_present("invalidate"));

            let mut rooms = std::collections::HashMap::new();

            for device in devices {
                let coordinator = device.coordinator().unwrap();

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
}

pub fn discover(pretty: bool, invalidate: bool) -> Vec<sonos::Speaker> {
    use serde::Serialize;

    const CACHE_FILE_NAME: &str = "/tmp/sonos-cli-speakers";

    if !invalidate {
        if let Ok(cache) = std::fs::File::open(CACHE_FILE_NAME) {
            let cache: Vec<IpAddr> = serde_json::from_reader(std::io::BufReader::new(cache))
                                                .unwrap();

            return cache.iter()
                .map(|i| sonos::Speaker::from_ip(*i).unwrap())
                .collect();
        }
    }

    if pretty {
        std::thread::spawn(|| {
            use std::io::{Write, stdout};

            const TWO: &str = "\u{23F2}\u{FE0F}  Give me 2 secs to discover your devices...";
            const ONE: &str = "\u{23F2}\u{FE0F}  Give me a sec to discover your devices...";

            print!("{}\r", TWO);
            stdout().flush().unwrap();

            std::thread::sleep(std::time::Duration::from_millis(1000));

            print!("{}{}\r", ONE, " ".repeat(TWO.len() - ONE.len()));
            stdout().flush().unwrap();

            std::thread::sleep(std::time::Duration::from_millis(999));

            print!("{}\r", " ".repeat(TWO.len()));
            stdout().flush().unwrap();
        });
    }

    let speakers = sonos::discover().unwrap();

    {
        // write IP addresses of all known speakers to cache
        let writer = std::fs::File::create(CACHE_FILE_NAME).unwrap();
        let mut serializer = serde_json::Serializer::new(writer);

        speakers.iter()
            .map(|s| s.ip)
            .collect::<Vec<IpAddr>>()
            .serialize(&mut serializer).unwrap();
    }

    speakers
}

#[derive(Serialize, Deserialize, Debug)]
struct Track {
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub running_time: std::time::Duration,
    pub duration: std::time::Duration
}

impl Track {
    pub fn new(speaker: &Speaker) -> Result<Track, failure::Error> {
        let track = speaker.track()?;

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

        let running_time = duration_to_hms(self.running_time);
        let duration = duration_to_hms(self.duration);

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
    pub fn new(vol: u8, muted: bool) -> Volume {
        Self {
            volume: vol,
            muted,
        }
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
