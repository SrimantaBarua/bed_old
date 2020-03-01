// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use std::{thread, time};

mod core;
mod textbuffer;
mod types;
mod ui;

const WIDTH: u32 = 800;
const HEIGHT: u32 = 600;
const TITLE: &str = "bed";

fn main() {
    let args = parse_args();

    let mut core = core::Core::new();
    let buffer = match args.value_of("FILE") {
        Some(path) => core
            .new_buffer_from_file(path)
            .expect("failed to open file"),
        None => core.new_empty_buffer(),
    };

    let (mut ui_core, window, events) = ui::UICore::init(core, buffer, WIDTH, HEIGHT, TITLE);
    let mut windows = vec![(window, events, (0, 0))];

    let target_duration = time::Duration::from_nanos(1_000_000_000 / 60);

    while windows.len() > 0 {
        let start = time::Instant::now();

        ui_core.poll_events();
        windows.retain(|(window, _, _)| !window.should_close());

        for i in 0..windows.len() {
            let (window, events, last_scroll) = &mut windows[i];
            let (should_refresh, cur_scroll) = window.handle_events(events, *last_scroll);
            if should_refresh {
                window.refresh();
            }
            windows[i].2 = cur_scroll;
        }

        let end = time::Instant::now();
        let diff = end - start;
        if diff < target_duration {
            thread::sleep(target_duration - diff);
        }
    }
}

fn parse_args<'a>() -> clap::ArgMatches<'a> {
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
