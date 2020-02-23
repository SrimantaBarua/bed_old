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
    let mut core = core::Core::new();
    let buffer = core.new_empty_buffer();
    let (mut ui_core, window, events) = ui::UICore::init(core, buffer, WIDTH, HEIGHT, TITLE);
    let mut windows = vec![(window, events)];

    let target_duration = time::Duration::from_nanos(1_000_000_000 / 30);

    while windows.len() > 0 {
        let start = time::Instant::now();

        ui_core.poll_events();
        windows.retain(|(window, _)| !window.should_close());

        for (window, events) in &mut windows {
            if window.handle_events(events) {
                window.refresh();
            }
        }

        let end = time::Instant::now();
        let diff = end - start;
        if diff < target_duration {
            thread::sleep(target_duration - diff);
        }
    }
}
