// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::Receiver;

use glfw::{Glfw, OpenGlProfileHint, WindowEvent, WindowHint};

use crate::core::Core;
use crate::textbuffer::Buffer;

mod context;
mod opengl;
mod quad;
mod types;
mod window;

#[derive(Clone)]
pub(crate) struct UICore {
    inner: Rc<RefCell<UICoreInner>>,
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
        // Create core and first window
        let ui_core = UICore {
            inner: Rc::new(RefCell::new(UICoreInner {
                glfw: glfw,
                core: core,
            })),
        };
        let (window, events) =
            window::Window::first_window(ui_core.inner.clone(), first_buffer, width, height, title);
        (ui_core, window, events)
    }

    pub(crate) fn poll_events(&mut self) {
        let inner = &mut *self.inner.borrow_mut();
        inner.glfw.poll_events();
    }
}

struct UICoreInner {
    glfw: Glfw,
    core: Core,
}
