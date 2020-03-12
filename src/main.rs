// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use std::cell::RefCell;
use std::rc::Rc;
use std::{thread, time};

mod config;
mod core;
mod font;
mod syntax;
mod textbuffer;
mod types;
mod ui;

const WIDTH: u32 = 800;
const HEIGHT: u32 = 600;
const TITLE: &str = "bed";

fn main() {
    let args = parse_args();

    // Initialize fonts
    let font_core = Rc::new(RefCell::new(
        font::FontCore::new().expect("failed to initialize font core"),
    ));
    let config = {
        let fc = &mut *font_core.borrow_mut();
        Rc::new(RefCell::new(config::Cfg::load(fc)))
    };

    let (mut ui_core, window, events) =
        ui::UICore::init(args, font_core, config, WIDTH, HEIGHT, TITLE);
    let mut windows = vec![(window, events, time::Instant::now())];

    let target_duration = time::Duration::from_nanos(1_000_000_000 / 60);

    while windows.len() > 0 {
        let start = time::Instant::now();
        ui_core.poll_events();
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

        let diff = start.elapsed();
        if diff < target_duration {
            thread::sleep(target_duration - diff);
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
