// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::Receiver;
use std::time;

use euclid::{point2, size2, Point2D, Rect, Size2D};
use glfw::{Action, Context, Glfw, Key, Modifiers, WindowEvent, WindowMode};

use crate::core::Core;
use crate::textbuffer::Buffer;
use crate::types::{Color, PixelSize, TextSize};

use super::context::RenderCtx;
use super::font::{FaceKey, FontCore};
use super::text::TextCursorStyle;
use super::textview::TextView;

static GUTTER_PADDING: u32 = 10;
static GUTTER_TEXTSIZE: f32 = 7.0;
static GUTTER_FG_COLOR: Color = Color::new(176, 176, 176, 255);
static GUTTER_BG_COLOR: Color = Color::new(255, 255, 255, 255);
static TEXTVIEW_BG_COLOR: Color = Color::new(255, 255, 255, 255);
static CLEAR_COLOR: Color = Color::new(255, 255, 255, 255);
static CURSOR_COLOR: Color = Color::new(255, 128, 0, 196);

#[cfg(target_os = "unix")]
const FIXED_FONT: &'static str = "monospace";
#[cfg(target_os = "windows")]
const FIXED_FONT: &'static str = "Consolas";

#[cfg(target_os = "unix")]
const VARIABLE_FONT: &'static str = "sans";
#[cfg(target_os = "windows")]
const VARIABLE_FONT: &'static str = "Arial";

// Horrible workaround because can't get things to work right on Windows
#[cfg(target_os = "windows")]
fn get_titlebar_height() -> u32 {
    30
}

#[cfg(not(target_os = "windows"))]
fn get_titlebar_height() -> u32 {
    0
}

pub(crate) struct Window {
    window: glfw::Window,
    render_ctx: RenderCtx,
    glfw: Rc<RefCell<Glfw>>,
    core: Rc<RefCell<Core>>,
    fixed_face: FaceKey,
    variable_face: FaceKey,
    textview: TextView,
    textview_scroll_v: (f64, f64),
    input_state: InputState,
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
            window.set_refresh_polling(true);
            window.set_framebuffer_size_polling(true);
            window.set_size(width as i32, height as i32);
            gl::load_with(|s| glfw.get_proc_address_raw(s));
            // Return stuff
            (window, events, dpi)
        };
        // Workaround for correct height (damn you Windows)
        let height = height - get_titlebar_height();
        // Initialie fonts
        let (fixed_face, variable_face) = {
            let fc = &mut *font_core.borrow_mut();
            let fixed_face = fc.find(FIXED_FONT).expect("failed to get monospace font");
            let variable_face = fc.find(VARIABLE_FONT).expect("failed to get sans font");
            (fixed_face, variable_face)
        };
        // Request view ID from core
        let view_id = (&mut *core.borrow_mut()).next_view_id();
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
            CURSOR_COLOR,
            TextCursorStyle::Block,
            view_id,
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
                textview_scroll_v: (0.0, 0.0),
                input_state: InputState::default(),
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
        let mut textview_scroll_a = (0.0, 0.0);

        // Apply friction
        self.textview_scroll_v.0 = self.textview_scroll_v.0 * (3.0 / 8.0);
        self.textview_scroll_v.1 = self.textview_scroll_v.1 * (7.0 / 8.0);

        for (_, event) in glfw::flush_messages(events) {
            to_refresh = true;
            match event {
                WindowEvent::FramebufferSize(w, h) => {
                    self.resize(size2(w as u32, h as u32 - get_titlebar_height()))
                }
                WindowEvent::Scroll(x, y) => {
                    // Scroll acceleration accumulation
                    textview_scroll_a.0 -= x;
                    textview_scroll_a.1 -= y;
                }
                e => self.handle_event(e),
            }
        }

        // Apply accelation
        let millis = duration.subsec_millis() as i32;
        self.textview_scroll_v.0 += (millis as f64 * textview_scroll_a.0) / 12.0;
        self.textview_scroll_v.1 += (millis as f64 * textview_scroll_a.1) / 12.0;

        // Calculate delta
        let textview_scroll_sx = (millis as f64 * self.textview_scroll_v.0) / 4.0;
        let textview_scroll_sy = (millis as f64 * self.textview_scroll_v.1) / 4.0;
        let textview_scroll_s = (textview_scroll_sx, textview_scroll_sy);

        // If there is any velocity, we need to refresh
        let textview_scroll_s = (
            textview_scroll_s.0.round() as i32,
            textview_scroll_s.1.round() as i32,
        );
        if textview_scroll_s != (0, 0) {
            to_refresh = true;
            self.textview.scroll(textview_scroll_s);
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

    pub(crate) fn set_should_close(&mut self, val: bool) {
        self.window.set_should_close(val);
    }

    fn resize(&mut self, size: Size2D<u32, PixelSize>) {
        self.render_ctx.set_size(size);
        self.textview.set_rect(Rect::new(point2(0, 0), size));
    }

    fn handle_event(&mut self, event: WindowEvent) {
        let state = &mut self.input_state;
        let textview = &mut self.textview;
        match state.mode {
            InputMode::Insert => match event {
                WindowEvent::Key(Key::Escape, _, Action::Press, _) => {
                    let mult = state.get_action_multiplier();
                    state.movement_multiplier.clear();
                    state.mode = InputMode::Normal;
                    state.last_edit = EditOp::Insert(mult, Insert(state.cur_insert_ops.clone()));
                    for _ in 0..(mult - 1) {
                        for op in &state.cur_insert_ops {
                            match op {
                                InsertOp::Str(s) => textview.insert_str(s),
                                InsertOp::Backspace => textview.delete_left(1),
                                InsertOp::Delete => textview.delete_right(1),
                                InsertOp::Left => textview.move_cursor_left(1),
                                InsertOp::Right => textview.move_cursor_right(1),
                                InsertOp::Up => textview.move_cursor_up(1),
                                InsertOp::Down => textview.move_cursor_down(1),
                                InsertOp::Home => textview.move_cursor_start_of_line(),
                                InsertOp::End => textview.move_cursor_end_of_line(),
                                InsertOp::PageUp => textview.page_up(),
                                InsertOp::PageDown => textview.page_down(),
                            }
                        }
                    }
                    self.input_state.cur_insert_ops.clear();
                    textview.set_cursor_style(TextCursorStyle::Block);
                }
                WindowEvent::Key(Key::Down, _, Action::Press, _)
                | WindowEvent::Key(Key::Down, _, Action::Repeat, _) => {
                    state.cur_insert_ops.push(InsertOp::Down);
                    textview.move_cursor_down(1);
                }
                WindowEvent::Key(Key::Up, _, Action::Press, _)
                | WindowEvent::Key(Key::Up, _, Action::Repeat, _) => {
                    state.cur_insert_ops.push(InsertOp::Up);
                    textview.move_cursor_up(1);
                }
                WindowEvent::Key(Key::Left, _, Action::Press, _)
                | WindowEvent::Key(Key::Left, _, Action::Repeat, _) => {
                    state.cur_insert_ops.push(InsertOp::Left);
                    textview.move_cursor_left(1);
                }
                WindowEvent::Key(Key::Right, _, Action::Press, _)
                | WindowEvent::Key(Key::Right, _, Action::Repeat, _) => {
                    state.cur_insert_ops.push(InsertOp::Right);
                    textview.move_cursor_right(1);
                }
                WindowEvent::Key(Key::Home, _, Action::Press, _)
                | WindowEvent::Key(Key::Home, _, Action::Repeat, _) => {
                    state.cur_insert_ops.push(InsertOp::Home);
                    textview.move_cursor_start_of_line();
                }
                WindowEvent::Key(Key::End, _, Action::Press, _)
                | WindowEvent::Key(Key::End, _, Action::Repeat, _) => {
                    state.cur_insert_ops.push(InsertOp::End);
                    textview.move_cursor_end_of_line();
                }
                WindowEvent::Key(Key::PageUp, _, Action::Press, _)
                | WindowEvent::Key(Key::PageUp, _, Action::Repeat, _) => {
                    state.cur_insert_ops.push(InsertOp::PageUp);
                    textview.page_up();
                }
                WindowEvent::Key(Key::PageDown, _, Action::Press, _)
                | WindowEvent::Key(Key::PageDown, _, Action::Repeat, _) => {
                    state.cur_insert_ops.push(InsertOp::PageDown);
                    textview.page_down();
                }
                WindowEvent::Key(Key::Backspace, _, Action::Press, _)
                | WindowEvent::Key(Key::Backspace, _, Action::Repeat, _) => {
                    state.cur_insert_ops.push(InsertOp::Backspace);
                    textview.delete_left(1);
                }
                WindowEvent::Key(Key::Delete, _, Action::Press, _)
                | WindowEvent::Key(Key::Delete, _, Action::Repeat, _) => {
                    state.cur_insert_ops.push(InsertOp::Delete);
                    textview.delete_right(1);
                }
                WindowEvent::Key(Key::V, _, Action::Press, m) => {
                    if m == Modifiers::Control | Modifiers::Shift {
                        if let Some(s) = self.window.get_clipboard_string() {
                            textview.insert_str(&s);
                        }
                    }
                }
                WindowEvent::Key(Key::Enter, _, Action::Press, _)
                | WindowEvent::Key(Key::Enter, _, Action::Repeat, _) => {
                    match state.cur_insert_ops.pop() {
                        Some(InsertOp::Str(mut s)) => {
                            s.push('\n');
                            state.cur_insert_ops.push(InsertOp::Str(s));
                        }
                        Some(o) => {
                            state.cur_insert_ops.push(o);
                            state.cur_insert_ops.push(InsertOp::Str("\n".to_owned()));
                        }
                        _ => state.cur_insert_ops.push(InsertOp::Str("\n".to_owned())),
                    }
                    textview.insert_char('\n');
                }
                WindowEvent::Key(Key::Tab, _, Action::Press, _)
                | WindowEvent::Key(Key::Tab, _, Action::Repeat, _) => {
                    match state.cur_insert_ops.pop() {
                        Some(InsertOp::Str(mut s)) => {
                            s.push('\t');
                            state.cur_insert_ops.push(InsertOp::Str(s));
                        }
                        Some(o) => {
                            state.cur_insert_ops.push(o);
                            state.cur_insert_ops.push(InsertOp::Str("\t".to_owned()));
                        }
                        _ => state.cur_insert_ops.push(InsertOp::Str("\t".to_owned())),
                    }
                    textview.insert_char('\t');
                }
                WindowEvent::Char(c) => {
                    match state.cur_insert_ops.pop() {
                        Some(InsertOp::Str(mut s)) => {
                            s.push(c);
                            state.cur_insert_ops.push(InsertOp::Str(s));
                        }
                        Some(o) => {
                            state.cur_insert_ops.push(o);
                            state.cur_insert_ops.push(InsertOp::Str(c.to_string()));
                        }
                        _ => state.cur_insert_ops.push(InsertOp::Str(c.to_string())),
                    }
                    textview.insert_char(c);
                }
                _ => {}
            },
            InputMode::Normal => match event {
                WindowEvent::Key(Key::Escape, _, Action::Press, _) => {
                    state.action_multiplier.clear();
                    state.movement_multiplier.clear();
                    self.set_should_close(true);
                }
                WindowEvent::Key(Key::Down, _, Action::Press, _)
                | WindowEvent::Key(Key::Down, _, Action::Repeat, _) => {
                    let mult = state.get_action_multiplier();
                    state.movement_multiplier.clear();
                    textview.move_cursor_down(mult);
                }
                WindowEvent::Key(Key::Up, _, Action::Press, _)
                | WindowEvent::Key(Key::Up, _, Action::Repeat, _) => {
                    let mult = state.get_action_multiplier();
                    state.movement_multiplier.clear();
                    textview.move_cursor_up(mult);
                }
                WindowEvent::Key(Key::Left, _, Action::Press, _)
                | WindowEvent::Key(Key::Left, _, Action::Repeat, _) => {
                    let mult = state.get_action_multiplier();
                    state.movement_multiplier.clear();
                    textview.move_cursor_left(mult);
                }
                WindowEvent::Key(Key::Right, _, Action::Press, _)
                | WindowEvent::Key(Key::Right, _, Action::Repeat, _) => {
                    let mult = state.get_action_multiplier();
                    state.movement_multiplier.clear();
                    textview.move_cursor_right(mult);
                }
                WindowEvent::Key(Key::Home, _, Action::Press, _)
                | WindowEvent::Key(Key::Home, _, Action::Repeat, _) => {
                    state.action_multiplier.clear();
                    state.movement_multiplier.clear();
                    textview.move_cursor_start_of_line();
                }
                WindowEvent::Key(Key::End, _, Action::Press, _)
                | WindowEvent::Key(Key::End, _, Action::Repeat, _) => {
                    state.action_multiplier.clear();
                    state.movement_multiplier.clear();
                    textview.move_cursor_end_of_line();
                }
                WindowEvent::Key(Key::PageUp, _, Action::Press, _)
                | WindowEvent::Key(Key::PageUp, _, Action::Repeat, _) => {
                    state.action_multiplier.clear();
                    state.movement_multiplier.clear();
                    textview.page_up();
                }
                WindowEvent::Key(Key::PageDown, _, Action::Press, _)
                | WindowEvent::Key(Key::PageDown, _, Action::Repeat, _) => {
                    state.action_multiplier.clear();
                    state.movement_multiplier.clear();
                    textview.page_down();
                }
                WindowEvent::Key(Key::Delete, _, Action::Press, _)
                | WindowEvent::Key(Key::Delete, _, Action::Repeat, _) => {
                    let mult = state.get_action_multiplier();
                    state.movement_multiplier.clear();
                    textview.delete_right(mult);
                }
                WindowEvent::Char('h') => {
                    let mult = state.get_action_multiplier();
                    state.movement_multiplier.clear();
                    textview.move_cursor_left(mult);
                }
                WindowEvent::Char('j') => {
                    let mult = state.get_action_multiplier();
                    state.movement_multiplier.clear();
                    textview.move_cursor_down(mult);
                }
                WindowEvent::Char('k') => {
                    let mult = state.get_action_multiplier();
                    state.movement_multiplier.clear();
                    textview.move_cursor_up(mult);
                }
                WindowEvent::Char('l') => {
                    let mult = state.get_action_multiplier();
                    state.movement_multiplier.clear();
                    textview.move_cursor_right(mult);
                }
                WindowEvent::Char('0') if state.action_multiplier.len() == 0 => {
                    state.action_multiplier.clear();
                    state.movement_multiplier.clear();
                    textview.move_cursor_start_of_line();
                }
                WindowEvent::Char('$') => {
                    state.action_multiplier.clear();
                    state.movement_multiplier.clear();
                    textview.move_cursor_end_of_line();
                }
                WindowEvent::Char('g') => {
                    let mut linum = state.get_action_multiplier();
                    if linum > 0 {
                        linum -= 1;
                    }
                    state.movement_multiplier.clear();
                    textview.go_to_line(linum);
                }
                WindowEvent::Char('G') => {
                    state.action_multiplier.clear();
                    state.movement_multiplier.clear();
                    textview.go_to_last_line();
                }
                WindowEvent::Char('d') => {
                    state.mode = InputMode::DeleteMotion;
                    textview.set_cursor_style(TextCursorStyle::Underline);
                }
                WindowEvent::Char('i') => {
                    state.mode = InputMode::Insert;
                    state.cur_insert_ops.clear();
                    textview.set_cursor_style(TextCursorStyle::Beam);
                }
                WindowEvent::Char('I') => {
                    state.mode = InputMode::Insert;
                    state.cur_insert_ops.clear();
                    state.cur_insert_ops.push(InsertOp::Home);
                    textview.set_cursor_style(TextCursorStyle::Beam);
                    textview.move_cursor_start_of_line();
                }
                WindowEvent::Char('a') => {
                    state.mode = InputMode::Insert;
                    state.cur_insert_ops.clear();
                    state.cur_insert_ops.push(InsertOp::Right);
                    textview.set_cursor_style(TextCursorStyle::Beam);
                    textview.move_cursor_right(1);
                }
                WindowEvent::Char('A') => {
                    state.mode = InputMode::Insert;
                    state.cur_insert_ops.clear();
                    state.cur_insert_ops.push(InsertOp::End);
                    textview.set_cursor_style(TextCursorStyle::Beam);
                    textview.move_cursor_end_of_line();
                }
                WindowEvent::Char('o') => {
                    state.mode = InputMode::Insert;
                    state.cur_insert_ops.clear();
                    state.cur_insert_ops.push(InsertOp::End);
                    state.cur_insert_ops.push(InsertOp::Str("\n".to_owned()));
                    textview.set_cursor_style(TextCursorStyle::Beam);
                    textview.move_cursor_end_of_line();
                    textview.insert_char('\n');
                }
                WindowEvent::Char('O') => {
                    state.mode = InputMode::Insert;
                    state.cur_insert_ops.clear();
                    state.cur_insert_ops.push(InsertOp::Home);
                    state.cur_insert_ops.push(InsertOp::Str("\n".to_owned()));
                    state.cur_insert_ops.push(InsertOp::Up);
                    textview.set_cursor_style(TextCursorStyle::Beam);
                    textview.move_cursor_start_of_line();
                    textview.insert_char('\n');
                    textview.move_cursor_up(1);
                }
                WindowEvent::Char('x') => {
                    let mult = state.get_action_multiplier();
                    state.movement_multiplier.clear();
                    state.last_edit = EditOp::DelChar(mult);
                    textview.delete_right(mult);
                }
                WindowEvent::Char('.') => {
                    let amul = state.get_action_multiplier();
                    state.movement_multiplier.clear();
                    match &state.last_edit {
                        EditOp::DelChar(n) => {
                            textview.delete_right(amul * *n);
                        }
                        EditOp::Delete(amul, movop) => match movop {
                            MovementOp::Default(mmul) => textview.delete_lines(amul * mmul),
                            MovementOp::Left(mmul) => textview.delete_left(amul * mmul),
                            MovementOp::Right(mmul) => textview.delete_right(amul * mmul),
                            MovementOp::Up(mmul) => textview.delete_lines_up(amul * mmul),
                            MovementOp::Down(mmul) => textview.delete_lines_down(amul * mmul),
                            MovementOp::Linum(mmul) => {
                                for _ in 0..*amul {
                                    textview.delete_to_line(*mmul);
                                }
                            }
                            MovementOp::LastLine => {
                                for _ in 0..*amul {
                                    textview.delete_to_last_line();
                                }
                            }
                            MovementOp::LineStart => textview.delete_to_line_start(),
                            MovementOp::LineEnd => textview.delete_to_line_end(),
                            _ => {}
                        },
                        EditOp::Insert(n, i) => {
                            textview.set_cursor_style(TextCursorStyle::Beam);
                            for _ in 0..(amul * *n) {
                                for op in &i.0 {
                                    match op {
                                        InsertOp::Str(s) => textview.insert_str(s),
                                        InsertOp::Backspace => textview.delete_left(1),
                                        InsertOp::Delete => textview.delete_right(1),
                                        InsertOp::Left => textview.move_cursor_left(1),
                                        InsertOp::Right => textview.move_cursor_right(1),
                                        InsertOp::Up => textview.move_cursor_up(1),
                                        InsertOp::Down => textview.move_cursor_down(1),
                                        InsertOp::Home => textview.move_cursor_start_of_line(),
                                        InsertOp::End => textview.move_cursor_end_of_line(),
                                        InsertOp::PageUp => textview.page_up(),
                                        InsertOp::PageDown => textview.page_down(),
                                    }
                                }
                            }
                            textview.set_cursor_style(TextCursorStyle::Block);
                        }
                        _ => {}
                    }
                }
                WindowEvent::Char(c) if c.is_digit(10) => {
                    state.action_multiplier.push(c);
                }
                _ => {}
            },
            InputMode::DeleteMotion => match event {
                WindowEvent::Key(Key::Escape, _, Action::Press, _) => {
                    state.action_multiplier.clear();
                    state.movement_multiplier.clear();
                    state.mode = InputMode::Normal;
                    textview.set_cursor_style(TextCursorStyle::Block);
                }
                WindowEvent::Char('h') => {
                    let act_mult = state.get_action_multiplier();
                    let move_mult = state.get_movement_multiplier();
                    state.last_edit = EditOp::Delete(act_mult, MovementOp::Left(move_mult));
                    state.mode = InputMode::Normal;
                    textview.set_cursor_style(TextCursorStyle::Block);
                    textview.delete_left(act_mult * move_mult);
                }
                WindowEvent::Char('l') => {
                    let act_mult = state.get_action_multiplier();
                    let move_mult = state.get_movement_multiplier();
                    state.last_edit = EditOp::Delete(act_mult, MovementOp::Right(move_mult));
                    state.mode = InputMode::Normal;
                    textview.set_cursor_style(TextCursorStyle::Block);
                    textview.delete_right(act_mult * move_mult);
                }
                WindowEvent::Char('j') => {
                    let act_mult = state.get_action_multiplier();
                    let move_mult = state.get_movement_multiplier();
                    state.last_edit = EditOp::Delete(act_mult, MovementOp::Down(move_mult));
                    state.mode = InputMode::Normal;
                    textview.set_cursor_style(TextCursorStyle::Block);
                    textview.delete_lines_down(act_mult * move_mult);
                }
                WindowEvent::Char('k') => {
                    let act_mult = state.get_action_multiplier();
                    let move_mult = state.get_movement_multiplier();
                    state.last_edit = EditOp::Delete(act_mult, MovementOp::Up(move_mult));
                    state.mode = InputMode::Normal;
                    textview.set_cursor_style(TextCursorStyle::Block);
                    textview.delete_lines_up(act_mult * move_mult);
                }
                WindowEvent::Char('0') if state.action_multiplier.len() == 0 => {
                    state.action_multiplier.clear();
                    state.movement_multiplier.clear();
                    state.last_edit = EditOp::Delete(1, MovementOp::LineStart);
                    state.mode = InputMode::Normal;
                    textview.set_cursor_style(TextCursorStyle::Block);
                    textview.delete_to_line_start();
                }
                WindowEvent::Char('$') => {
                    state.action_multiplier.clear();
                    state.movement_multiplier.clear();
                    state.last_edit = EditOp::Delete(1, MovementOp::LineEnd);
                    state.mode = InputMode::Normal;
                    textview.set_cursor_style(TextCursorStyle::Block);
                    textview.delete_to_line_end();
                }
                WindowEvent::Char('g') => {
                    let act_mult = state.get_action_multiplier();
                    let mut linum = state.get_movement_multiplier();
                    if linum > 0 {
                        linum -= 1;
                    }
                    state.last_edit = EditOp::Delete(act_mult, MovementOp::Linum(linum));
                    state.mode = InputMode::Normal;
                    textview.set_cursor_style(TextCursorStyle::Block);
                    for _ in 0..act_mult {
                        textview.delete_to_line(linum);
                    }
                }
                WindowEvent::Char('G') => {
                    let act_mult = state.get_action_multiplier();
                    state.movement_multiplier.clear();
                    state.last_edit = EditOp::Delete(act_mult, MovementOp::LastLine);
                    state.mode = InputMode::Normal;
                    textview.set_cursor_style(TextCursorStyle::Block);
                    for _ in 0..act_mult {
                        textview.delete_to_last_line();
                    }
                }
                WindowEvent::Char('d') => {
                    let act_mult = state.get_action_multiplier();
                    let move_mult = state.get_movement_multiplier();
                    state.last_edit = EditOp::Delete(act_mult, MovementOp::Default(move_mult));
                    state.mode = InputMode::Normal;
                    textview.set_cursor_style(TextCursorStyle::Block);
                    textview.delete_lines(act_mult * move_mult);
                }
                WindowEvent::Char(c) if c.is_digit(10) => {
                    state.movement_multiplier.push(c);
                }
                _ => {}
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InputMode {
    Insert,
    Normal,
    DeleteMotion,
}

impl Default for InputMode {
    fn default() -> InputMode {
        InputMode::Normal
    }
}

#[derive(Debug)]
struct InputState {
    mode: InputMode,
    action_multiplier: String,
    movement_multiplier: String,
    cur_insert_ops: Vec<InsertOp>,
    last_edit: EditOp,
}

impl Default for InputState {
    fn default() -> InputState {
        InputState {
            mode: InputMode::default(),
            action_multiplier: String::new(),
            movement_multiplier: String::new(),
            cur_insert_ops: Vec::new(),
            last_edit: EditOp::None,
        }
    }
}

impl InputState {
    fn get_movement_multiplier(&mut self) -> usize {
        if self.movement_multiplier.len() == 0 {
            1
        } else {
            let ret = self.movement_multiplier.parse().unwrap_or(1);
            self.movement_multiplier.clear();
            ret
        }
    }

    fn get_action_multiplier(&mut self) -> usize {
        if self.action_multiplier.len() == 0 {
            1
        } else {
            let ret = self.action_multiplier.parse().unwrap_or(1);
            self.action_multiplier.clear();
            ret
        }
    }
}

#[derive(Debug)]
struct Insert(Vec<InsertOp>);

#[derive(Clone, Debug)]
enum InsertOp {
    Str(String),
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
}

#[derive(Debug)]
enum EditOp {
    None,
    Delete(usize, MovementOp),
    Change(usize, MovementOp),
    DelChar(usize),
    SubstChar(usize),
    Insert(usize, Insert),
}

#[derive(Debug, Eq, PartialEq)]
enum MovementOp {
    Default(usize),
    Left(usize),
    Right(usize),
    Up(usize),
    Down(usize),
    LastLine,
    LineStart,
    LineEnd,
    NextWord,
    PrevWord,
    NextEnd,
    NextMajorWord,
    PrevMajorWord,
    NextMajorEnd,
    Linum(usize),
}
