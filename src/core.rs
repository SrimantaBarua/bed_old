// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use std::io::Result as IOResult;

use crate::textbuffer::Buffer;

pub(crate) struct Core {
    buffers: Vec<Buffer>,
}

impl Core {
    pub(crate) fn new() -> Core {
        Core {
            buffers: Vec::new(),
        }
    }

    pub(crate) fn new_empty_buffer(&mut self) -> Buffer {
        let buffer = Buffer::empty();
        self.buffers.push(buffer.clone());
        buffer
    }

    pub(crate) fn new_buffer_from_file(&mut self, path: &str) -> IOResult<Buffer> {
        Buffer::from_file(path).map(|b| {
            self.buffers.push(b.clone());
            b
        })
    }
}
