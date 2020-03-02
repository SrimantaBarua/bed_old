// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use std::cell::RefCell;
use std::io::Result as IOResult;
use std::rc::Rc;

use crate::textbuffer::Buffer;

const TABSIZE: usize = 8;

pub(crate) struct Core {
    buffers: Vec<Rc<RefCell<Buffer>>>,
}

impl Core {
    pub(crate) fn new() -> Core {
        Core {
            buffers: Vec::new(),
        }
    }

    pub(crate) fn new_empty_buffer(&mut self) -> Rc<RefCell<Buffer>> {
        let buffer = Rc::new(RefCell::new(Buffer::empty(TABSIZE)));
        self.buffers.push(buffer.clone());
        buffer
    }

    pub(crate) fn new_buffer_from_file(&mut self, path: &str) -> IOResult<Rc<RefCell<Buffer>>> {
        Buffer::from_file(path, TABSIZE).map(|b| {
            let b = Rc::new(RefCell::new(b));
            self.buffers.push(b.clone());
            b
        })
    }
}
