// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::Receiver;

use glfw::{Glfw, OpenGlProfileHint, WindowEvent, WindowHint};

use crate::core::Core;
use crate::textbuffer::Buffer;

mod context;
mod font;
mod glyphrender;
mod opengl;
mod quad;
mod window;

use font::FontCore;
use window::Window;

#[derive(Clone)]
pub(crate) struct UICore {
    glfw: Rc<RefCell<Glfw>>,
    core: Rc<RefCell<Core>>,
    font_core: Rc<RefCell<FontCore>>,
}

impl UICore {
    pub(crate) fn init(
        core: Core,
        first_buffer: Buffer,
        width: u32,
        height: u32,
        title: &str,
    ) -> (UICore, window::Window, Receiver<(f64, WindowEvent)>) {
        // Initialize GLFW
        let mut glfw = glfw::init(glfw::FAIL_ON_ERRORS).expect("failed to initialize GLFW");
        glfw.window_hint(WindowHint::ContextVersion(3, 3));
        glfw.window_hint(WindowHint::OpenGlProfile(OpenGlProfileHint::Core));
        // Initialize fonts
        let font_core = Rc::new(RefCell::new(
            FontCore::new().expect("failed to initialize font core"),
        ));
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
            first_buffer,
            width,
            height,
            title,
        );
        (ui_core, window, events)
    }

    pub(crate) fn poll_events(&mut self) {
        let glfw = &mut *self.glfw.borrow_mut();
        glfw.poll_events();
    }
}
