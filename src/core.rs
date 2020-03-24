// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Result as IOResult;
use std::rc::Rc;

use euclid::Size2D;

use crate::config::Cfg;
use crate::font::FontCore;
use crate::textbuffer::Buffer;
use crate::types::DPI;

pub(crate) struct Core {
    buffers: HashMap<String, Rc<RefCell<Buffer>>>,
    font_core: Rc<RefCell<FontCore>>,
    config: Rc<RefCell<Cfg>>,
    next_view_id: usize,
}

impl Core {
    pub(crate) fn new(font_core: Rc<RefCell<FontCore>>, config: Rc<RefCell<Cfg>>) -> Core {
        Core {
            buffers: HashMap::new(),
            next_view_id: 0,
            font_core: font_core,
            config: config,
        }
    }

    pub(crate) fn new_empty_buffer(&mut self, dpi: Size2D<u32, DPI>) -> Rc<RefCell<Buffer>> {
        Rc::new(RefCell::new(Buffer::empty(
            dpi,
            self.font_core.clone(),
            self.config.clone(),
        )))
    }

    pub(crate) fn new_buffer_from_file(
        &mut self,
        path: &str,
        dpi: Size2D<u32, DPI>,
    ) -> IOResult<Rc<RefCell<Buffer>>> {
        if let Some(buffer) = self.buffers.get_mut(path) {
            {
                let buffer = &mut *buffer.borrow_mut();
                buffer.reload_from_file(dpi)?;
            }
            Ok(buffer.clone())
        } else {
            let buffer = Rc::new(RefCell::new(Buffer::from_file(
                path,
                dpi,
                self.font_core.clone(),
                self.config.clone(),
            )));
            self.buffers.insert(path.to_owned(), buffer.clone());
            Ok(buffer)
        }
    }

    pub(crate) fn next_view_id(&mut self) -> usize {
        let ret = self.next_view_id;
        self.next_view_id += 1;
        ret
    }
}
