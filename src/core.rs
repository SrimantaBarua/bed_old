// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Result as IOResult;
use std::rc::Rc;

use euclid::Size2D;

use crate::textbuffer::Buffer;
use crate::types::DPI;
use crate::ui::font::{FaceKey, FontCore};

const TABSIZE: usize = 8;

pub(crate) struct Core {
    buffers: HashMap<String, Rc<RefCell<Buffer>>>,
    fixed_face: FaceKey,
    variable_face: FaceKey,
    font_core: Rc<RefCell<FontCore>>,
    next_view_id: usize,
}

impl Core {
    pub(crate) fn new(
        fixed_face: FaceKey,
        variable_face: FaceKey,
        font_core: Rc<RefCell<FontCore>>,
    ) -> Core {
        Core {
            buffers: HashMap::new(),
            next_view_id: 0,
            fixed_face: fixed_face,
            variable_face: variable_face,
            font_core: font_core,
        }
    }

    pub(crate) fn new_empty_buffer(&mut self, dpi: Size2D<u32, DPI>) -> Rc<RefCell<Buffer>> {
        Rc::new(RefCell::new(Buffer::empty(
            TABSIZE,
            dpi,
            self.fixed_face,
            self.variable_face,
            self.font_core.clone(),
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
            let buffer = Buffer::from_file(
                path,
                TABSIZE,
                dpi,
                self.fixed_face,
                self.variable_face,
                self.font_core.clone(),
            )
            .map(|b| Rc::new(RefCell::new(b)))?;
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
