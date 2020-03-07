// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use std::time;

mod core;
mod textbuffer;
mod types;
mod ui;

const WIDTH: u32 = 800;
const HEIGHT: u32 = 600;
const TITLE: &str = "bed";

fn main() {
    let args = parse_args();

    let (mut ui_core, window, events) = ui::UICore::init(args, WIDTH, HEIGHT, TITLE);
    let mut windows = vec![(window, events, time::Instant::now())];

    let target_duration = time::Duration::from_nanos(1_000_000_000 / 60).as_secs_f64();

    while windows.len() > 0 {
        ui_core.wait_events(target_duration);
        windows.retain(|(window, _, _)| !window.should_close());

        for i in 0..windows.len() {
            let (window, events, last_time) = &mut windows[i];
            let cur_time = time::Instant::now();
            let should_refresh = window.handle_events(events, cur_time - *last_time);
            if should_refresh {
                window.refresh();
            }
            windows[i].2 = cur_time;
        }
    }
}

fn parse_args() -> clap::ArgMatches<'static> {
    use clap::{App, Arg};
    App::new("bed")
        .version("0.0.1")
        .author("Srimanta Barua <srimanta.barua1@gmail.com>")
        .about("Barua's editor")
        .arg(
            Arg::with_name("FILE")
                .help("file to open")
                .required(false)
                .index(1),
        )
        .get_matches()
}
