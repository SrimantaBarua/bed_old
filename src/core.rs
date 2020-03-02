// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use std::cell::RefCell;
use std::io::Result as IOResult;
use std::rc::Rc;

use crate::textbuffer::Buffer;

const TABSIZE: usize = 8;

pub(crate) struct Core {
    buffers: Vec<Rc<RefCell<Buffer>>>,
    next_view_id: usize,
}

impl Core {
    pub(crate) fn new() -> Core {
        Core {
            buffers: Vec::new(),
            next_view_id: 0,
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

    pub(crate) fn next_view_id(&mut self) -> usize {
        let ret = self.next_view_id;
        self.next_view_id += 1;
        ret
    }
}
