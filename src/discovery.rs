use std::net::IpAddr;
use std::time::Duration;

use sonos::Speaker;
use failure::Fallible;

use tokio::io::{self, AsyncWriteExt, AsyncReadExt};
use futures::future::try_join_all;

pub async fn find_speaker_by_name(name: &str) -> Fallible<Speaker> {
    let mut speakers = discover(true, false).await?;

    let mut min = 100;

    speakers.sort_by(|a, b| {
        let a = strsim::damerau_levenshtein(&a.name, name);
        let b = strsim::damerau_levenshtein(&b.name, name);

        if a < min { min = a; }
        if b < min { min = b; }

        a.cmp(&b)
    });

    if min > 5 {
        bail!("Couldn't find a speaker by that name");
    }

    let speaker = speakers.remove(0);

    if min > 2 {
        let mut stdin = io::stdin();
        let mut stdout = io::stdout();

        stdout.write_all(format!("Couldn't find speaker '{}', did you mean {}? [Y/n] ", name, speaker.name).as_bytes()).await?;
        stdout.flush().await?;

        let input = stdin
            .read_u8()
            .await? as char;

        if input != 'y' && input != 'Y' {
            bail!("Couldn't find a speaker by that name");
        }
    }

    Ok(speaker)
}

pub async fn discover(pretty: bool, invalidate: bool) -> Fallible<Vec<Speaker>> {
    use serde::Serialize;

    const CACHE_FILE_NAME: &str = "/tmp/sonos-cli-speakers";

    if !invalidate {
        if let Ok(mut cache) = tokio::fs::File::open(CACHE_FILE_NAME).await {
            let mut contents: Vec<u8> = vec![];
            cache.read_to_end(&mut contents).await?;

            let cache: Vec<IpAddr> = serde_json::from_slice(contents.as_ref())?;

            return try_join_all(cache.into_iter().map(Speaker::from_ip)).await;
        }
    }

    if pretty {
        tokio::spawn(async {
            let mut stdout = io::stdout();

            const TWO: &str = "\u{23F2}\u{FE0F}  Give me 2 secs to discover your devices...";
            const ONE: &str = "\u{23F2}\u{FE0F}  Give me a sec to discover your devices...";

            stdout.write_all(TWO.as_bytes()).await?;
            stdout.write_all(b"\r").await?;
            stdout.flush().await?;

            tokio::time::delay_for(Duration::from_millis(1000)).await;

            stdout.write_all(ONE.as_bytes()).await?;
            stdout.write_all(" ".repeat(TWO.len() - ONE.len()).as_bytes()).await?;
            stdout.write_all(b"\r").await?;
            stdout.flush().await?;

            tokio::time::delay_for(Duration::from_millis(999)).await;

            stdout.write_all(" ".repeat(TWO.len()).as_bytes()).await?;
            stdout.write_all(b"\r").await?;
            stdout.flush().await?;

            Ok::<(), failure::Error>(())
        });
    }

    let speakers = sonos::discover().await?;

    {
        // write IP addresses of all known speakers to cache
        let writer = std::fs::File::create(CACHE_FILE_NAME).unwrap();
        let mut serializer = serde_json::Serializer::new(writer);

        speakers.iter()
            .map(|s| s.ip)
            .collect::<Vec<IpAddr>>()
            .serialize(&mut serializer).unwrap();
    }

    Ok(speakers)
}
