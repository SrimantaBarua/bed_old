// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc::channel;
use std::sync::mpsc::Receiver;
use std::{thread, time};

use directories::BaseDirs;
#[cfg(target_os = "windows")]
use euclid::SideOffsets2D;
use euclid::{point2, size2, Rect, Size2D};
use glfw::{Action, Context, Glfw, Key, Modifiers, WindowEvent, WindowMode};
use walkdir::WalkDir;

use crate::config::Cfg;
use crate::core::Core;
use crate::types::{Color, PixelSize};

use super::context::RenderCtx;
use super::fuzzy_popup::FuzzyPopup;
use super::prompt::Prompt;
use super::text::TextCursorStyle;
use super::textview_tree::TextViewTree;
use crate::font::FontCore;

static CLEAR_COLOR: Color = Color::new(255, 255, 255, 255);

// Because windows messes things up, we have to get viewable region
#[cfg(not(target_os = "windows"))]
fn get_viewable_rect(window: &glfw::Window) -> Rect<u32, PixelSize> {
    let (w, h) = window.get_framebuffer_size();
    Rect::new(point2(0, 0), size2(w, h)).cast()
}

#[cfg(target_os = "windows")]
fn get_viewable_rect(window: &glfw::Window) -> Rect<u32, PixelSize> {
    let (w, h) = window.get_framebuffer_size();
    let rect = Rect::new(point2(0, 0), size2(w, h));
    let (l, t, r, b) = window.get_frame_size();
    let off = SideOffsets2D::new(t, r, b, l);
    rect.inner_rect(off).cast()
}

#[cfg(not(target_os = "windows"))]
fn scale_point_to_viewable(_window: &glfw::Window, point: (f64, f64)) -> (f64, f64) {
    point
}

#[cfg(target_os = "windows")]
fn scale_point_to_viewable(window: &glfw::Window, point: (f64, f64)) -> (f64, f64) {
    let (w, h) = window.get_framebuffer_size();
    let vrect = get_viewable_rect(window);
    (
        point.0 * vrect.size.width as f64 / w as f64,
        point.1 * vrect.size.height as f64 / h as f64,
    )
}

pub(crate) struct Window {
    window: glfw::Window,
    render_ctx: RenderCtx,
    glfw: Rc<RefCell<Glfw>>,
    core: Rc<RefCell<Core>>,
    textview_tree: TextViewTree,
    prompt: Prompt,
    fuzzy_popup: FuzzyPopup,
    input_state: InputState,
    font_core: Rc<RefCell<FontCore>>,
    working_directory: PathBuf,
}

impl Window {
    pub(super) fn first_window(
        glfw: Rc<RefCell<Glfw>>,
        core: Rc<RefCell<Core>>,
        font_core: Rc<RefCell<FontCore>>,
        config: Rc<RefCell<Cfg>>,
        first_buffer_path: Option<&str>,
        width: u32,
        height: u32,
        title: &str,
    ) -> (Window, Receiver<(f64, WindowEvent)>) {
        let (mut window, events, dpi) = {
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
            window.set_mouse_button_polling(true);
            // Return stuff
            (window, events, dpi)
        };
        // Open first buffer
        let buffer = {
            let core = &mut *core.borrow_mut();
            match first_buffer_path {
                Some(spath) => {
                    let path = Path::new(spath);
                    if path.is_absolute() {
                        core.new_buffer_from_file(spath, dpi)
                            .expect("failed to open file")
                    } else {
                        let mut working_directory =
                            std::env::current_dir().expect("failed to get current directory");
                        working_directory.push(path);
                        let spath = working_directory
                            .to_str()
                            .expect("failed to convert path to string");
                        core.new_buffer_from_file(spath, dpi)
                            .expect("failed to open file")
                    }
                }
                None => core.new_empty_buffer(dpi),
            }
        };
        // Request view ID from core
        let view_id = (&mut *core.borrow_mut()).next_view_id();
        // Initialize text view tree
        let inner_rect = get_viewable_rect(&window);
        let textview_tree = TextViewTree::new(
            buffer,
            inner_rect,
            font_core.clone(),
            config.clone(),
            dpi,
            true,
            false,
            view_id,
        );
        // Initialize fuzzy search popup
        let fuzzy_popup = FuzzyPopup::new(inner_rect, font_core.clone(), config.clone(), dpi);
        // Initialize editor prompt
        let prompt = Prompt::new(inner_rect, font_core.clone(), config, dpi);
        // Make window visible
        window.show();
        // Return window wrapper
        let ctx = RenderCtx::new(&mut window, size2(width, height), dpi, CLEAR_COLOR);
        (
            Window {
                window: window,
                render_ctx: ctx,
                glfw: glfw,
                core: core,
                textview_tree: textview_tree,
                fuzzy_popup: fuzzy_popup,
                prompt: prompt,
                input_state: InputState::default(),
                font_core: font_core,
                working_directory: std::env::current_dir()
                    .expect("failed to get current directory"),
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
        let mut scroll_force = (0.0, 0.0);
        let mut cursor_position = None;
        let time = duration.as_secs_f64() * 100.0;

        for (_, event) in glfw::flush_messages(events) {
            to_refresh = true;
            match event {
                WindowEvent::FramebufferSize(w, h) => self.resize(size2(w as u32, h as u32)),
                WindowEvent::MouseButton(glfw::MouseButtonLeft, Action::Press, _) => {
                    let point = self.window.get_cursor_pos();
                    // windows-only scale
                    let (x, y) = scale_point_to_viewable(&self.window, point);
                    self.textview_tree
                        .move_cursor_to_point((x as i32, y as i32));
                }
                WindowEvent::Scroll(ax, ay) => {
                    // Get cursor position
                    let point = self.window.get_cursor_pos();
                    let (x, y) = scale_point_to_viewable(&self.window, point);
                    cursor_position = Some((x as i32, y as i32));
                    // Scroll acceleration accumulation
                    scroll_force.0 -= ax;
                    scroll_force.1 -= ay;
                }
                e => self.handle_event(e),
            }
            if self.should_close() {
                break;
            }
        }

        // If any view was scrolled, refresh
        to_refresh |= self
            .textview_tree
            .scroll_views(cursor_position, scroll_force, time);

        // Update fuzzy finder async if required
        if self.fuzzy_popup.is_active() {
            self.fuzzy_popup.update_from_async();
            to_refresh |= self.fuzzy_popup.to_refresh;
        }

        to_refresh
    }

    pub(crate) fn refresh(&mut self) {
        let mut active_ctx = self.render_ctx.activate(&mut self.window);
        active_ctx.clear();
        self.textview_tree.draw(&mut active_ctx);

        if self.fuzzy_popup.is_active() {
            self.fuzzy_popup.draw(&mut active_ctx);
        }
        if self.prompt.is_active() {
            self.prompt.draw(&mut active_ctx);
        }

        self.window.swap_buffers();
    }

    pub(crate) fn should_close(&self) -> bool {
        self.window.should_close()
    }

    pub(crate) fn set_should_close(&mut self, val: bool) {
        self.window.set_should_close(val);
    }

    fn handle_command(&mut self) {
        let prompt_s = self.prompt.get_string().trim();
        let mut iter = prompt_s.split_whitespace();
        match iter.next() {
            Some(":q") | Some(":quit") => {
                self.prompt.set_active(false);
                self.input_state.mode = InputMode::Normal;
                if self.textview_tree.kill_active() {
                    self.set_should_close(true);
                }
            }
            Some(":bn") | Some(":bnext") => {
                self.textview_tree.active_mut().next_buffer();
                self.prompt.set_active(false);
                self.input_state.mode = InputMode::Normal;
            }
            Some(":bp") | Some(":bprevious") => {
                self.textview_tree.active_mut().prev_buffer();
                self.prompt.set_active(false);
                self.input_state.mode = InputMode::Normal;
            }
            Some(":e") | Some(":edit") => match iter.next() {
                Some(fname) => {
                    let core = &mut *self.core.borrow_mut();
                    let path = Path::new(fname);
                    let path = if path.has_root() {
                        path.to_path_buf()
                    } else if path.starts_with("~") {
                        let path = path.strip_prefix("~").unwrap();
                        let mut buf = BaseDirs::new()
                            .expect("failed to get base dirs")
                            .home_dir()
                            .to_path_buf();
                        buf.push(path);
                        buf
                    } else {
                        let mut buf = self.working_directory.clone();
                        buf.push(path);
                        buf
                    };
                    match core.new_buffer_from_file(path.to_str().unwrap(), self.render_ctx.dpi) {
                        Ok(buffer) => {
                            let view_id = core.next_view_id();
                            self.textview_tree.active_mut().add_buffer(buffer, view_id);
                        }
                        Err(e) => {
                            eprintln!("failed to open file: {:?}: {}", path, e);
                        }
                    }
                    self.prompt.set_active(false);
                    self.input_state.mode = InputMode::Normal;
                }
                _ => {
                    self.textview_tree
                        .active_mut()
                        .reload_buffer()
                        .expect("failed to reload buffer");
                    self.prompt.set_active(false);
                    self.input_state.mode = InputMode::Normal;
                }
            },
            Some(":vsp") | Some(":vsplit") => {
                let core = &mut *self.core.borrow_mut();
                self.textview_tree.split_h(core.next_view_id());
                self.prompt.set_active(false);
                self.input_state.mode = InputMode::Normal;
            }
            Some(":sp") | Some(":split") => {
                let core = &mut *self.core.borrow_mut();
                self.textview_tree.split_v(core.next_view_id());
                self.prompt.set_active(false);
                self.input_state.mode = InputMode::Normal;
            }
            Some(":w") | Some(":write") => {
                let res = if let Some(fname) = iter.next() {
                    let path = Path::new(fname);
                    let path = if path.has_root() {
                        path.to_path_buf()
                    } else if path.starts_with("~") {
                        let path = path.strip_prefix("~").unwrap();
                        let mut buf = BaseDirs::new()
                            .expect("failed to get base dirs")
                            .home_dir()
                            .to_path_buf();
                        buf.push(path);
                        buf
                    } else {
                        let mut buf = self.working_directory.clone();
                        buf.push(path);
                        buf
                    };
                    self.textview_tree.active_mut().write_buffer(Some(
                        path.to_str()
                            .expect("failed to get text representation of path"),
                    ))
                } else {
                    self.textview_tree.active_mut().write_buffer(None)
                };
                match res {
                    Some(Err(e)) => {
                        eprintln!("failed to write buffer: {}", e);
                    }
                    None => {
                        eprintln!("no path provided for writing buffer");
                    }
                    _ => {}
                }
                self.prompt.set_active(false);
                self.input_state.mode = InputMode::Normal;
            }
            Some(":fzf") => {
                self.fuzzy_popup.set_active(true);
                self.fuzzy_popup.set_default_on_empty(true);
                let wdir = self.working_directory.clone();
                let basename = wdir.file_name().and_then(|p| p.to_str()).unwrap_or("/");
                self.fuzzy_popup.set_input_label(basename);
                let (tx, rx) = channel();
                thread::spawn(move || {
                    for e in WalkDir::new(&wdir)
                        .into_iter()
                        .filter_entry(|e| {
                            e.file_name()
                                .to_str()
                                .map(|s| !s.starts_with("."))
                                .unwrap_or(true)
                        })
                        .filter_map(|e| e.ok())
                    {
                        let mut path = e.path();
                        if path.is_file() {
                            path = path.strip_prefix(&wdir).unwrap();
                            if let Some(path) = path.to_str().map(|s| s.to_string()) {
                                if tx.send(path).is_err() {
                                    break;
                                }
                            }
                        }
                    }
                });
                self.fuzzy_popup.set_async_source(rx);
                self.fuzzy_popup.update_from_async();
                self.prompt.set_active(false);
                self.input_state.mode = InputMode::Fuzzy;
            }
            _ => {}
        }
    }

    fn handle_fuzzy(&mut self) {
        if let Some(selection) = self.fuzzy_popup.get_selection() {
            let core = &mut *self.core.borrow_mut();
            let mut path = self.working_directory.clone();
            path.push(&selection);
            match core.new_buffer_from_file(path.to_str().unwrap(), self.render_ctx.dpi) {
                Ok(buffer) => {
                    let view_id = core.next_view_id();
                    self.textview_tree.active_mut().add_buffer(buffer, view_id);
                }
                Err(e) => {
                    println!("failed to open file: {:?}: {}", path, e);
                }
            }
        }
        self.fuzzy_popup.set_active(false);
        self.input_state.mode = InputMode::Normal;
    }

    fn resize(&mut self, size: Size2D<u32, PixelSize>) {
        let vrect = get_viewable_rect(&self.window);
        self.render_ctx.set_size(size);
        self.textview_tree.set_rect(vrect);
        self.fuzzy_popup.set_window_rect(vrect);
        self.prompt.set_window_rect(vrect);
    }

    fn handle_event(&mut self, event: WindowEvent) {
        let mut state = &mut self.input_state;
        let textview = self.textview_tree.active_mut();
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
                WindowEvent::Char(':') => {
                    state.action_multiplier.clear();
                    state.movement_multiplier.clear();
                    state.mode = InputMode::Command;
                    self.prompt.set_active(true);
                    self.prompt.set_string(":");
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
            InputMode::Command => match event {
                WindowEvent::Key(Key::Escape, _, Action::Press, _) => {
                    state.mode = InputMode::Normal;
                    self.prompt.set_active(false);
                }
                WindowEvent::Char(c) => {
                    self.prompt.insert(c);
                }
                WindowEvent::Key(Key::Up, _, Action::Press, _)
                | WindowEvent::Key(Key::Up, _, Action::Repeat, _) => {
                    self.prompt.up_key();
                }
                WindowEvent::Key(Key::Down, _, Action::Press, _)
                | WindowEvent::Key(Key::Down, _, Action::Repeat, _) => {
                    self.prompt.down_key();
                }
                WindowEvent::Key(Key::Left, _, Action::Press, _)
                | WindowEvent::Key(Key::Left, _, Action::Repeat, _) => {
                    self.prompt.left_key();
                }
                WindowEvent::Key(Key::Right, _, Action::Press, _)
                | WindowEvent::Key(Key::Right, _, Action::Repeat, _) => {
                    self.prompt.right_key();
                }
                WindowEvent::Key(Key::Tab, _, Action::Press, _)
                | WindowEvent::Key(Key::Tab, _, Action::Repeat, _) => {
                    // TODO: Completions
                }
                WindowEvent::Key(Key::Backspace, _, Action::Press, _)
                | WindowEvent::Key(Key::Backspace, _, Action::Repeat, _) => {
                    self.prompt.delete_left();
                    if self.prompt.get_string().len() == 0 {
                        self.prompt.set_active(false);
                        state.mode = InputMode::Normal;
                    }
                }
                WindowEvent::Key(Key::Enter, _, Action::Press, _) => {
                    self.handle_command();
                    self.prompt.push_to_history();
                }
                _ => {}
            },
            InputMode::Fuzzy => match event {
                WindowEvent::Key(Key::Escape, _, Action::Press, _) => {
                    state.mode = InputMode::Normal;
                    self.fuzzy_popup.set_active(false);
                }
                WindowEvent::Char(c) => {
                    self.fuzzy_popup.insert(c);
                    self.fuzzy_popup.re_filter();
                }
                WindowEvent::Key(Key::Tab, _, Action::Press, _)
                | WindowEvent::Key(Key::Tab, _, Action::Repeat, _) => {
                    self.fuzzy_popup.tab_key();
                    self.fuzzy_popup.re_filter();
                }
                WindowEvent::Key(Key::Backspace, _, Action::Press, _)
                | WindowEvent::Key(Key::Backspace, _, Action::Repeat, _) => {
                    self.fuzzy_popup.delete_left();
                    self.fuzzy_popup.re_filter();
                }
                WindowEvent::Key(Key::Up, _, Action::Press, _)
                | WindowEvent::Key(Key::Up, _, Action::Repeat, _) => {
                    self.fuzzy_popup.up_key();
                }
                WindowEvent::Key(Key::Down, _, Action::Press, _)
                | WindowEvent::Key(Key::Down, _, Action::Repeat, _) => {
                    self.fuzzy_popup.down_key();
                }
                WindowEvent::Key(Key::Enter, _, Action::Press, _) => {
                    self.handle_fuzzy();
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
                WindowEvent::Char('0') if state.movement_multiplier.len() == 0 => {
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
    Command,
    Fuzzy,
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
