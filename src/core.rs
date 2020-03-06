// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Result as IOResult;
use std::rc::Rc;

use crate::textbuffer::Buffer;

const TABSIZE: usize = 8;

pub(crate) struct Core {
    buffers: HashMap<String, Rc<RefCell<Buffer>>>,
    next_view_id: usize,
}

impl Core {
    pub(crate) fn new() -> Core {
        Core {
            buffers: HashMap::new(),
            next_view_id: 0,
        }
    }

    pub(crate) fn new_empty_buffer(&mut self) -> Rc<RefCell<Buffer>> {
        Rc::new(RefCell::new(Buffer::empty(TABSIZE)))
    }

    pub(crate) fn new_buffer_from_file(&mut self, path: &str) -> IOResult<Rc<RefCell<Buffer>>> {
        println!("path: {}", path);
        if let Some(buffer) = self.buffers.get_mut(path) {
            {
                let buffer = &mut *buffer.borrow_mut();
                buffer.reload_from_file()?;
            }
            Ok(buffer.clone())
        } else {
            let buffer = Buffer::from_file(path, TABSIZE).map(|b| Rc::new(RefCell::new(b)))?;
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
