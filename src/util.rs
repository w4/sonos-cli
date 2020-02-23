macro_rules! print_struct {
    ($args:ident, $struc:expr) => {{
        info!("{}", if $args.is_present("json") {
            serde_json::to_string($struc)?
        } else {
            $struc.to_string()
        })
    }}
}

pub fn duration_to_hms(d: std::time::Duration) -> String {
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

pub fn setup_logger() -> Result<(), fern::InitError> {
    fern::Dispatch::new()
        .format(|out, message, _record| {
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