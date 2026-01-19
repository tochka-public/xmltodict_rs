use crate::error::pyerr_to_io;
use crate::reader::pending::PendingBytes;
use pyo3::prelude::*;
use pyo3::types::{PyByteArray, PyBytes, PyMemoryView, PyString};
use std::io::{self, Read};

pub struct PyGeneratorRead {
    generator: Py<PyAny>,
    pending: PendingBytes,
    done: bool,
    bytearray_buffer: Option<Vec<u8>>,
}

impl PyGeneratorRead {
    pub fn new(generator: Py<PyAny>) -> Self {
        Self {
            generator,
            pending: PendingBytes::default(),
            done: false,
            bytearray_buffer: None,
        }
    }

    fn next_non_empty_chunk<'py>(
        &mut self,
        py: Python<'py>,
    ) -> io::Result<Option<Bound<'py, PyAny>>> {
        while !self.done {
            let generator = self.generator.bind(py);
            let chunk = match generator.call_method0("__next__") {
                Ok(chunk) => chunk,
                Err(err) => {
                    if err.is_instance_of::<pyo3::exceptions::PyStopIteration>(py) {
                        self.done = true;
                        return Ok(None);
                    }
                    return Err(pyerr_to_io(&err));
                }
            };

            if chunk.is_none() {
                return Err(pyerr_to_io(
                    &PyErr::new::<pyo3::exceptions::PyTypeError, _>(
                        "a bytes-like object or str is required, not 'NoneType'",
                    ),
                ));
            }

            if let Ok(chunk_str) = chunk.downcast::<PyString>() {
                if !chunk_str
                    .to_str()
                    .map_err(|err| pyerr_to_io(&err))?
                    .is_empty()
                {
                    return Ok(Some(chunk));
                }
                continue;
            }

            if let Ok(chunk_bytes) = chunk.downcast::<PyBytes>() {
                if !chunk_bytes.as_bytes().is_empty() {
                    return Ok(Some(chunk));
                }
                continue;
            }

            if let Ok(chunk_bytearray) = chunk.downcast::<PyByteArray>() {
                let bytes_vec = chunk_bytearray.to_vec();
                if !bytes_vec.is_empty() {
                    self.bytearray_buffer = Some(bytes_vec);
                    return Ok(Some(chunk));
                }
                continue;
            }

            if let Ok(chunk_memview) = chunk.downcast::<PyMemoryView>() {
                let bytes_obj = chunk_memview
                    .call_method0("tobytes")
                    .map_err(|err| pyerr_to_io(&err))?;
                let bytes = bytes_obj.downcast::<PyBytes>().map_err(|_| {
                    pyerr_to_io(&PyErr::new::<pyo3::exceptions::PyTypeError, _>(
                        "a bytes-like object or str is required, not 'memoryview'",
                    ))
                })?;
                if !bytes.as_bytes().is_empty() {
                    return Ok(Some(bytes_obj));
                }
                continue;
            }

            let bytes = chunk.extract::<&[u8]>().map_err(|err| pyerr_to_io(&err))?;
            if !bytes.is_empty() {
                return Ok(Some(chunk));
            }
        }
        Ok(None)
    }
}

impl Read for PyGeneratorRead {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if out.is_empty() {
            return Ok(0);
        }

        if !self.pending.is_empty() {
            return Ok(self.pending.copy_into(out));
        }

        if self.done {
            return Ok(0);
        }

        Python::attach(|py| {
            let Some(chunk) = self.next_non_empty_chunk(py)? else {
                return Ok(0);
            };

            if let Ok(chunk_str) = chunk.downcast::<PyString>() {
                let text = chunk_str.to_str().map_err(|err| pyerr_to_io(&err))?;
                let bytes = text.as_bytes();

                if bytes.len() <= out.len() {
                    let Some(dst) = out.get_mut(..bytes.len()) else {
                        return Err(io::Error::other("Internal buffer error"));
                    };
                    dst.copy_from_slice(bytes);
                    return Ok(bytes.len());
                }

                let out_len = out.len();
                let Some(src) = bytes.get(..out_len) else {
                    return Err(io::Error::other("Internal buffer error"));
                };
                out.copy_from_slice(src);
                let Some(rest) = bytes.get(out_len..) else {
                    return Err(io::Error::other("Internal buffer error"));
                };
                self.pending.fill_from_slice(rest);
                return Ok(out.len());
            }

            let bytes = if let Ok(chunk_bytes) = chunk.downcast::<PyBytes>() {
                chunk_bytes.as_bytes()
            } else if let Ok(chunk_bytearray) = chunk.downcast::<PyByteArray>() {
                self.bytearray_buffer = Some(chunk_bytearray.to_vec());
                if let Some(bytes_ref) = self.bytearray_buffer.as_deref() {
                    bytes_ref
                } else {
                    return Err(pyerr_to_io(
                        &PyErr::new::<pyo3::exceptions::PyTypeError, _>(
                            "a bytes-like object or str is required, not 'bytearray'",
                        ),
                    ));
                }
            } else {
                let type_name = chunk
                    .get_type()
                    .name()
                    .and_then(|n| n.extract::<String>())
                    .unwrap_or_else(|_| "unknown".to_string());
                return Err(pyerr_to_io(
                    &PyErr::new::<pyo3::exceptions::PyTypeError, _>(format!(
                        "a bytes-like object or str is required, not '{type_name}'"
                    )),
                ));
            };

            if bytes.len() <= out.len() {
                let Some(dst) = out.get_mut(..bytes.len()) else {
                    return Err(io::Error::other("Internal buffer error"));
                };
                dst.copy_from_slice(bytes);
                return Ok(bytes.len());
            }

            let out_len = out.len();
            let Some(src) = bytes.get(..out_len) else {
                return Err(io::Error::other("Internal buffer error"));
            };
            out.copy_from_slice(src);
            let Some(rest) = bytes.get(out_len..) else {
                return Err(io::Error::other("Internal buffer error"));
            };
            self.pending.fill_from_slice(rest);
            Ok(out.len())
        })
    }
}
