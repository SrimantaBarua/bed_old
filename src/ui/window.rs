// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::Receiver;
use std::time;

use euclid::{point2, size2, Rect, Size2D};
use glfw::{Context, Glfw, WindowEvent, WindowMode};

use crate::core::Core;
use crate::textbuffer::Buffer;
use crate::types::{Color, PixelSize, TextSize};

use super::context::RenderCtx;
use super::font::{FaceKey, FontCore};
use super::textview::TextView;

static GUTTER_PADDING: u32 = 10;
static GUTTER_TEXTSIZE: f32 = 7.0;
static GUTTER_FG_COLOR: Color = Color::new(176, 176, 176, 255);
static GUTTER_BG_COLOR: Color = Color::new(255, 255, 255, 255);
static TEXTVIEW_BG_COLOR: Color = Color::new(255, 255, 255, 255);
static CLEAR_COLOR: Color = Color::new(255, 255, 255, 255);

pub(crate) struct Window {
    window: glfw::Window,
    render_ctx: RenderCtx,
    glfw: Rc<RefCell<Glfw>>,
    core: Rc<RefCell<Core>>,
    fixed_face: FaceKey,
    variable_face: FaceKey,
    textview: TextView,
    textview_scroll_v: (i32, i32),
    font_core: Rc<RefCell<FontCore>>,
}

impl Window {
    pub(super) fn first_window(
        glfw: Rc<RefCell<Glfw>>,
        core: Rc<RefCell<Core>>,
        font_core: Rc<RefCell<FontCore>>,
        first_buffer: Rc<RefCell<Buffer>>,
        width: u32,
        height: u32,
        title: &str,
    ) -> (Window, Receiver<(f64, WindowEvent)>) {
        let (window, events, dpi) = {
            let glfw = &mut *glfw.borrow_mut();
            // Create GLFW window and calculate DPI
            let (mut window, events, dpi) = glfw.with_primary_monitor(|glfw, m| {
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
            gl::load_with(|s| glfw.get_proc_address_raw(s));
            // Return stuff
            (window, events, dpi)
        };
        // Initialie fonts
        let (fixed_face, variable_face) = {
            let fc = &mut *font_core.borrow_mut();
            let fixed_face = fc.find("monospace").expect("failed to get monospace font");
            let variable_face = fc.find("sans").expect("failed to get sans font");
            (fixed_face, variable_face)
        };
        // Initialize text view tree
        let rect = Rect::new(point2(0, 0), size2(width, height));
        let textview = TextView::new(
            first_buffer,
            rect,
            TEXTVIEW_BG_COLOR,
            fixed_face,
            variable_face,
            font_core.clone(),
            dpi,
            true,
            GUTTER_PADDING,
            TextSize::from_f32(GUTTER_TEXTSIZE),
            GUTTER_FG_COLOR,
            GUTTER_BG_COLOR,
        );
        // Return window wrapper
        (
            Window {
                window: window,
                render_ctx: RenderCtx::new(size2(width, height), dpi, CLEAR_COLOR),
                glfw: glfw,
                core: core,
                fixed_face: fixed_face,
                variable_face: variable_face,
                textview: textview,
                textview_scroll_v: (0, 0),
                font_core: font_core,
            },
            events,
        )
    }

    pub(crate) fn handle_events(
        &mut self,
        events: &Receiver<(f64, WindowEvent)>,
        duration: time::Duration,
    ) -> bool {
        let mut to_refresh = false;
        let mut textview_scroll_a = (0, 0, false);
        for (_, event) in glfw::flush_messages(events) {
            to_refresh = true;
            match event {
                WindowEvent::FramebufferSize(w, h) => self.resize(size2(w as u32, h as u32)),
                WindowEvent::Scroll(x, y) => {
                    let (x, y) = (-(x as i32), -(y as i32));
                    textview_scroll_a.0 += x;
                    textview_scroll_a.1 += y;
                    textview_scroll_a.2 = true;
                }
                _ => {}
            }
        }
        if !textview_scroll_a.2 {
            self.textview_scroll_v.0 /= 2;
            self.textview_scroll_v.1 /= 2;
        } else {
            let millis = duration.subsec_millis() as i32;
            self.textview_scroll_v.0 += (millis * textview_scroll_a.0) * 60 / 200;
            self.textview_scroll_v.1 += (millis * textview_scroll_a.1) * 60 / 200;
        }
        if self.textview_scroll_v != (0, 0) {
            to_refresh = true;
            self.textview.scroll(self.textview_scroll_v);
        }

        to_refresh
    }

    pub(crate) fn refresh(&mut self) {
        let mut active_ctx = self.render_ctx.activate(&mut self.window);
        active_ctx.clear();
        self.textview.draw(&mut active_ctx);
        self.window.swap_buffers();
    }

    pub(crate) fn should_close(&self) -> bool {
        self.window.should_close()
    }

    fn resize(&mut self, size: Size2D<u32, PixelSize>) {
        self.render_ctx.set_size(size);
        self.textview
            .set_rect(Rect::new(point2(0, 0), size2(size.width, size.height)));
    }
}
