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

use std::net::IpAddr;
use sonos::Speaker;

fn argparse<'a, 'b>() -> clap::App<'a, 'b> {
    use clap::{App, AppSettings, Arg, SubCommand};

    App::new("sonos")
        .version(crate_version!())
        .author("Jordan Doyle <jordan@9t9t9.com>")
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
        .subcommand(SubCommand::with_name("rooms").about("List all of your speakers"))
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
        // sonos::discover().unwrap().iter()
        //    .find(|d| d.name == controller)
        //    .unwrap()
        panic!("not implemented");
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
        ("rooms", _) => {
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

            let devices = sonos::discover();
            println!("{:#?}", devices);
        },
        _ => {
            panic!();
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct Track {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub running_time: std::time::Duration,
    pub duration: std::time::Duration
}

impl Track {
    pub fn new(speaker: &Speaker) -> Result<Track, sonos::Error> {
        let track = speaker.track()?;

        Ok(Track {
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
        writeln!(f, "\u{1F4BF}  {}", self.album)?;

        let running_time = duration_to_hms(self.running_time);
        let duration = duration_to_hms(self.duration);

        write!(f, "\u{23F1}\u{FE0F}  {}/{}", running_time, duration)?;

        const PROG_BAR_LEN: usize = 25;
        let percent_played = ((self.running_time.as_secs() as f64 / self.duration.as_secs() as f64) * PROG_BAR_LEN as f64) as usize;
        write!(f, " [{}{}]", "\u{2587}".repeat(percent_played), "-".repeat(PROG_BAR_LEN - percent_played))
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
