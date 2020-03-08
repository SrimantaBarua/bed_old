// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::Receiver;

use glfw::{Glfw, OpenGlProfileHint, WindowEvent, WindowHint};

use crate::core::Core;

mod context;
pub(crate) mod font;
mod fuzzy_popup;
mod glyphrender;
mod opengl;
mod quad;
pub(crate) mod text;
mod textview;
mod window;

use font::FontCore;
use window::Window;

#[cfg(target_os = "linux")]
const FIXED_FONT: &'static str = "monospace";
#[cfg(target_os = "windows")]
const FIXED_FONT: &'static str = "Consolas";

#[cfg(target_os = "linux")]
const VARIABLE_FONT: &'static str = "sans";
#[cfg(target_os = "windows")]
const VARIABLE_FONT: &'static str = "Arial";

#[derive(Clone)]
pub(crate) struct UICore {
    glfw: Rc<RefCell<Glfw>>,
    core: Rc<RefCell<Core>>,
    font_core: Rc<RefCell<FontCore>>,
}

impl UICore {
    pub(crate) fn init(
        args: clap::ArgMatches,
        width: u32,
        height: u32,
        title: &str,
    ) -> (UICore, window::Window, Receiver<(f64, WindowEvent)>) {
        // Initialize GLFW
        let mut glfw = glfw::init(glfw::FAIL_ON_ERRORS).expect("failed to initialize GLFW");
        glfw.window_hint(WindowHint::Visible(false));
        glfw.window_hint(WindowHint::ContextVersion(3, 3));
        glfw.window_hint(WindowHint::OpenGlProfile(OpenGlProfileHint::Core));
        // Initialize fonts
        let font_core = Rc::new(RefCell::new(
            FontCore::new().expect("failed to initialize font core"),
        ));
        // Get default fixed and variable width fonts
        let (fixed_face, variable_face) = {
            let fc = &mut *font_core.borrow_mut();
            let fixed_face = fc.find(FIXED_FONT).expect("failed to get monospace font");
            let variable_face = fc.find(VARIABLE_FONT).expect("failed to get sans font");
            (fixed_face, variable_face)
        };
        // Initialize editor core
        let core = Core::new(fixed_face, variable_face, font_core.clone());
        let first_buffer_path = args.value_of("FILE");
        // Create core and first window
        let ui_core = UICore {
            glfw: Rc::new(RefCell::new(glfw)),
            core: Rc::new(RefCell::new(core)),
            font_core: font_core,
        };
        let (window, events) = Window::first_window(
            ui_core.glfw.clone(),
            ui_core.core.clone(),
            ui_core.font_core.clone(),
            fixed_face,
            variable_face,
            first_buffer_path,
            width,
            height,
            title,
        );
        (ui_core, window, events)
    }

    pub(crate) fn wait_events(&mut self, timeout_sec: f64) {
        let glfw = &mut *self.glfw.borrow_mut();
        glfw.wait_events_timeout(timeout_sec);
    }
}
