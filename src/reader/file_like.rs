use crate::error::pyerr_to_io;
use crate::reader::pending::PendingBytes;
use pyo3::prelude::*;
use pyo3::types::{PyByteArray, PyBytes};
use std::io::{self, Read};

pub struct PyFileLikeRead {
    file_like: Py<PyAny>,
    pending: PendingBytes,
    bytearray_buffer: Option<Vec<u8>>,
}

impl PyFileLikeRead {
    pub fn new(file_like: Py<PyAny>) -> Self {
        Self {
            file_like,
            pending: PendingBytes::default(),
            bytearray_buffer: None,
        }
    }
}

impl Read for PyFileLikeRead {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if out.is_empty() {
            return Ok(0);
        }

        if !self.pending.is_empty() {
            return Ok(self.pending.copy_into(out));
        }

        Python::attach(|py| {
            let file_like = self.file_like.bind(py);
            let chunk = match file_like.call_method1("read", (out.len(),)) {
                Ok(chunk) => chunk,
                Err(err) => return Err(pyerr_to_io(&err)),
            };

            let bytes = if let Ok(chunk_bytes) = chunk.cast::<PyBytes>() {
                chunk_bytes.as_bytes()
            } else if let Ok(chunk_bytearray) = chunk.cast::<PyByteArray>() {
                self.bytearray_buffer = Some(chunk_bytearray.to_vec());
                self.bytearray_buffer.as_deref().unwrap_or(&[])
            } else {
                let type_name = chunk
                    .get_type()
                    .name()
                    .and_then(|n| n.extract::<String>())
                    .unwrap_or_else(|_| "unknown".to_owned());
                return Err(pyerr_to_io(
                    &PyErr::new::<pyo3::exceptions::PyTypeError, _>(format!(
                        "read() did not return a bytes object (type={type_name})"
                    )),
                ));
            };

            if bytes.is_empty() {
                return Ok(0);
            }

            Ok(self.pending.write_chunk(bytes, out))
        })
    }
}
