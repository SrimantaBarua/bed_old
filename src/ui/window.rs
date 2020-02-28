// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::Receiver;

use euclid::{point2, size2, Rect, Size2D};
use glfw::{Context, WindowEvent, WindowMode};

use crate::textbuffer::Buffer;

use super::context::RenderCtx;
use super::types::PixelSize;
use super::UICoreInner;

pub(crate) struct Window {
    window: glfw::Window,
    render_ctx: RenderCtx,
    ui_core_inner: Rc<RefCell<UICoreInner>>,
}

impl Window {
    pub(super) fn first_window(
        ui_core_inner: Rc<RefCell<UICoreInner>>,
        first_buffer: Buffer,
        width: u32,
        height: u32,
        title: &str,
    ) -> (Window, Receiver<(f64, WindowEvent)>) {
        let (window, events, dpi) = {
            let ui_core = &mut *ui_core_inner.borrow_mut();
            // Create GLFW window and calculate DPI
            let (mut window, events, dpi) = ui_core.glfw.with_primary_monitor(|glfw, m| {
                let (window, events) = glfw
                    .create_window(width, height, title, WindowMode::Windowed)
                    .expect("failed to create GLFW window");
                let dpi = m
                    .and_then(|m| {
                        const MM_IN: f32 = 0.0393701;
                        let (width_mm, height_mm) = m.get_physical_size();
                        let (width_in, height_in) =
                            (width_mm as f32 * MM_IN, height_mm as f32 * MM_IN);
                        m.get_video_mode().map(|vm| {
                            let (width_p, height_p) = (vm.width as f32, vm.height as f32);
                            size2((width_p / width_in) as u32, (height_p / height_in) as u32)
                        })
                    })
                    .unwrap_or(size2(96, 96));
                (window, events, dpi)
            });
            // Make window the current GL context and load OpenGL function pointers
            window.make_current();
            window.set_key_polling(true);
            window.set_char_polling(true);
            window.set_scroll_polling(true);
            window.set_framebuffer_size_polling(true);
            gl::load_with(|s| ui_core.glfw.get_proc_address_raw(s));
            // Return stuff
            (window, events, dpi)
        };
        // Return window wrapper
        let clear_color = crate::types::Color::new(255, 255, 255, 255);
        (
            Window {
                window: window,
                render_ctx: RenderCtx::new(size2(width, height), dpi, clear_color),
                ui_core_inner: ui_core_inner,
            },
            events,
        )
    }

    pub(crate) fn handle_events(&mut self, events: &Receiver<(f64, WindowEvent)>) -> bool {
        for (_, event) in glfw::flush_messages(events) {
            match event {
                WindowEvent::FramebufferSize(w, h) => self.resize(size2(w as u32, h as u32)),
                _ => {}
            }
        }
        true
    }

    pub(crate) fn refresh(&mut self) {
        let mut active_ctx = self.render_ctx.activate(&mut self.window);
        active_ctx.clear();
        self.window.swap_buffers();
    }

    pub(crate) fn should_close(&self) -> bool {
        self.window.should_close()
    }

    fn resize(&mut self, size: Size2D<u32, PixelSize>) {
        self.render_ctx.set_size(size);
    }
}
