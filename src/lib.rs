#[cfg(all(
    feature = "mimalloc",
    any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "windows", target_arch = "x86_64"),
        target_os = "macos"
    )
))]
use mimalloc::MiMalloc;

use pyo3::prelude::*;
use pyo3::types::{
    PyAny, PyByteArray, PyBytes, PyDict, PyList, PyMemoryView, PyModule, PyString, PyTuple, PyType,
};
use pyo3::IntoPyObjectExt;
use quick_xml::events::Event;
use quick_xml::name::PrefixDeclaration;
use quick_xml::Reader;
use std::borrow::Cow;
use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Read};
use std::slice::from_raw_parts;
use std::str::from_utf8_unchecked;

#[cfg(all(
    feature = "mimalloc",
    any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "windows", target_arch = "x86_64"),
        target_os = "macos"
    )
))]
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

const DEFAULT_NAMESPACE_NAME: &str = "";

#[allow(clippy::struct_excessive_bools)]
#[derive(Clone)]
pub struct ParseConfig {
    pub xml_attribs: bool,
    pub attr_prefix: String,
    pub cdata_key: String,
    pub force_cdata: bool,
    pub cdata_separator: String,
    pub strip_whitespace: bool,
    pub namespace_separator: String,
    pub process_namespaces: bool,
    pub process_comments: bool,
    pub comment_key: String,
    pub item_depth: usize,
    pub disable_entities: bool,
    pub namespaces: Option<HashMap<String, String>>,
}

impl Default for ParseConfig {
    fn default() -> Self {
        Self {
            xml_attribs: true,
            attr_prefix: "@".to_string(),
            cdata_key: "#text".to_string(),
            force_cdata: false,
            cdata_separator: String::new(),
            strip_whitespace: true,
            namespace_separator: ":".to_string(),
            process_namespaces: false,
            process_comments: false,
            comment_key: "#comment".to_string(),
            item_depth: 0,
            disable_entities: true,
            namespaces: None,
        }
    }
}

pub struct XmlParser {
    config: ParseConfig,
    force_list: Option<Py<PyAny>>,
    postprocessor: Option<Py<PyAny>>,
    stack: Vec<Py<PyAny>>,
    path: Vec<String>,
    text_stack: Vec<Vec<String>>,
    namespace_stack: Vec<HashMap<String, String>>,
}

impl XmlParser {
    #[must_use]
    pub fn new(
        config: ParseConfig,
        force_list: Option<Py<PyAny>>,
        postprocessor: Option<Py<PyAny>>,
    ) -> Self {
        Self {
            config,
            force_list,
            postprocessor,
            stack: Vec::new(),
            path: Vec::new(),
            text_stack: Vec::new(),
            namespace_stack: Vec::new(),
        }
    }

    fn should_force_list(&self, py: Python, key: &str, value: &Bound<'_, PyAny>) -> PyResult<bool> {
        let Some(force_list) = &self.force_list else {
            return Ok(false);
        };

        if let Ok(val) = force_list.extract::<bool>(py) {
            return Ok(val);
        }

        if let Ok(val) = force_list
            .call_method1(py, "__contains__", (key,))
            .and_then(|x| x.extract::<bool>(py))
        {
            return Ok(val);
        }

        if let Ok(path_list) = PyList::new(py, &self.path) {
            let callable_result = force_list.call1(py, (path_list, key, value))?;
            let bool_val = callable_result.extract::<bool>(py)?;
            return Ok(bool_val);
        }

        Ok(false)
    }

    #[inline]
    fn apply_postprocessor<'py>(
        &self,
        py: Python<'py>,
        key: &str,
        data: &Bound<'py, PyAny>,
    ) -> PyResult<Option<(String, Bound<'py, PyAny>)>> {
        let mut final_key = key.to_string();
        let mut final_value = data.clone();

        if let Some(proc) = &self.postprocessor {
            let path_list = PyList::new(py, &self.path)?;
            let result = proc.call1(py, (path_list, key, data))?;

            if result.is_none(py) {
                return Ok(None);
            }

            let tuple = result.bind(py).downcast::<PyTuple>()?;
            final_key = tuple.get_item(0)?.extract::<String>()?;
            final_value = tuple.get_item(1)?;
        }

        Ok(Some((final_key, final_value)))
    }

    fn push_data(
        &self,
        py: Python,
        item: &Bound<'_, PyDict>,
        key: &str,
        data: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let Some((final_key, final_value)) = self.apply_postprocessor(py, key, data)? else {
            return Ok(());
        };

        match item.get_item(final_key.as_str())? {
            Some(existing) => {
                if let Ok(list) = existing.downcast::<PyList>() {
                    list.append(data.clone())?;
                } else {
                    let new_list = PyList::new(py, [existing.clone(), final_value.clone()])?;
                    item.set_item(final_key, &new_list)?;
                }
            }
            None => {
                if self.should_force_list(py, final_key.as_str(), final_value.as_ref())? {
                    let new_list = PyList::new(py, [final_value.clone()])?;
                    item.set_item(final_key, &new_list)?;
                } else {
                    item.set_item(final_key, final_value)?;
                }
            }
        }

        Ok(())
    }

    fn build_name(&self, full_name: &str) -> String {
        if !self.config.process_namespaces {
            return full_name.to_string();
        }

        let Some(ns_map) = self.namespace_stack.last() else {
            return full_name.to_string();
        };
        let ns_sep = &self.config.namespace_separator;
        let (prefix, name) = full_name
            .split_once(':')
            .unwrap_or((DEFAULT_NAMESPACE_NAME, full_name));
        if let Some(uri) = ns_map.get(prefix) {
            let mapped = self
                .config
                .namespaces
                .as_ref()
                .and_then(|m| m.get(uri))
                .unwrap_or(uri);
            return format!("{mapped}{ns_sep}{name}");
        }
        full_name.to_string()
    }

    fn start_element(
        &mut self,
        py: Python,
        name: &str,
        attrs: &[quick_xml::events::attributes::Attribute],
    ) -> PyResult<()> {
        let mut current_ns_map = self.namespace_stack.last().cloned().unwrap_or_default();

        let element_dict = PyDict::new(py);
        let mut set_xmlns_item = false;
        let mut normal_attrs: Vec<(String, String)> = Vec::new();

        if self.config.xml_attribs && !attrs.is_empty() {
            for attr in attrs {
                let key = &attr.key;
                let value_string = std::str::from_utf8(attr.value.as_ref())?.to_string();

                if self.config.process_namespaces {
                    if let Some(ns) = key.as_namespace_binding() {
                        match ns {
                            PrefixDeclaration::Default => {
                                current_ns_map
                                    .insert(DEFAULT_NAMESPACE_NAME.to_string(), value_string);
                            }
                            PrefixDeclaration::Named(name) => {
                                let key_string = String::from_utf8(name.to_vec())?;
                                if !set_xmlns_item {
                                    set_xmlns_item = self
                                        .config
                                        .namespaces
                                        .as_ref()
                                        .is_none_or(|m| !m.contains_key(&key_string));
                                }
                                current_ns_map.insert(key_string, value_string);
                            }
                        }
                        continue;
                    }
                }

                normal_attrs.push((String::from_utf8(key.into_inner().to_vec())?, value_string));
            }
        }

        if self.config.xml_attribs && !normal_attrs.is_empty() && set_xmlns_item {
            let ns_py = PyDict::new(py);
            for (key, value) in &current_ns_map {
                ns_py.set_item(key, value)?;
            }
            let xmlns_key = format!("{}{}", self.config.attr_prefix, "xmlns");
            element_dict.set_item(xmlns_key, ns_py)?;
        }

        self.namespace_stack.push(current_ns_map);

        if self.config.xml_attribs {
            for (key, value) in normal_attrs {
                let attr_local_name = if self.config.process_namespaces
                    && key.contains(&self.config.namespace_separator)
                {
                    self.build_name(&key)
                } else {
                    key
                };

                let prefixed_key = format!("{}{}", self.config.attr_prefix, attr_local_name);
                let Some((final_key, final_value)) = self.apply_postprocessor(
                    py,
                    prefixed_key.as_str(),
                    value.into_py_any(py)?.bind(py),
                )?
                else {
                    continue;
                };
                element_dict.set_item(final_key, final_value)?;
            }
        }

        let element_name = if self.config.process_namespaces {
            self.build_name(name)
        } else {
            name.to_string()
        };

        self.stack.push(element_dict.into());
        self.path.push(element_name);
        self.text_stack.push(Vec::new());

        Ok(())
    }

    fn end_element(&mut self, py: Python, name: &str) -> PyResult<()> {
        let element_name = self.build_name(name);

        let Some(current_element) = self.stack.pop() else {
            return Err(expat_error(py, "unexpected closing tag".to_string()));
        };
        let Some(text_parts) = self.text_stack.pop() else {
            return Err(expat_error(py, "unexpected closing tag".to_string()));
        };
        let Some(_) = self.path.pop() else {
            return Err(expat_error(py, "unexpected closing tag".to_string()));
        };

        let text_content = if text_parts.is_empty() {
            None
        } else {
            let joined = text_parts.join(&self.config.cdata_separator);
            if self.config.strip_whitespace && joined.trim().is_empty() {
                None
            } else {
                Some(joined)
            }
        };

        let element_dict = current_element.downcast_bound::<PyDict>(py)?;
        let has_attrs = !element_dict.is_empty();

        let final_value = match (has_attrs, text_content) {
            (false, None) => py.None(),
            (false, Some(text)) => {
                if self.config.force_cdata {
                    let dict = PyDict::new(py);
                    if let Some((final_key, final_value)) = self.apply_postprocessor(
                        py,
                        &self.config.cdata_key,
                        text.into_py_any(py)?.bind(py),
                    )? {
                        dict.set_item(final_key, final_value)?;
                    }
                    dict.into()
                } else {
                    text.into_pyobject(py)?.into_any().unbind()
                }
            }
            (true, Some(text)) => {
                if let Some((final_key, final_value)) = self.apply_postprocessor(
                    py,
                    &self.config.cdata_key,
                    text.into_py_any(py)?.bind(py),
                )? {
                    element_dict.set_item(final_key, final_value)?;
                }
                current_element
            }
            (true, None) => current_element,
        };

        if self.stack.is_empty() {
            let result_dict = PyDict::new(py);
            let Some((final_key, final_value)) =
                self.apply_postprocessor(py, element_name.as_str(), final_value.bind(py))?
            else {
                return Ok(());
            };
            result_dict.set_item(final_key, final_value)?;
            self.stack.push(result_dict.into());
        } else {
            let Some(parent) = self.stack.last() else {
                return Err(expat_error(py, "unexpected closing tag".to_string()));
            };
            let parent_dict = parent.downcast_bound::<PyDict>(py)?;

            self.push_data(py, parent_dict, &element_name, final_value.bind(py))?;
        }

        let Some(_) = self.namespace_stack.pop() else {
            return Err(expat_error(py, "unexpected closing tag".to_string()));
        };

        Ok(())
    }

    fn characters(&mut self, data: &str) {
        if let Some(current_text) = self.text_stack.last_mut() {
            current_text.push(data.to_string());
        }
    }

    fn comment(&self, py: Python, comment: &str) -> PyResult<()> {
        let Some(parent) = self.stack.last() else {
            return Ok(());
        };
        let parent_dict = parent.downcast_bound::<PyDict>(py)?;
        let comment_py = if self.config.strip_whitespace {
            comment.trim().into_pyobject(py)?
        } else {
            comment.into_pyobject(py)?
        };
        self.push_data(py, parent_dict, &self.config.comment_key, &comment_py)
    }
}

/// Wrapper to store `PyErr` inside `io::Error` while preserving the original exception type.
/// `PyErr` is Send but not Sync, so we need unsafe impl Sync.
/// This is safe because we only access the inner `PyErr` while holding the GIL.
struct WrappedPyErr(PyErr);

impl std::fmt::Debug for WrappedPyErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("WrappedPyErr").finish()
    }
}

impl std::fmt::Display for WrappedPyErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Python::attach(|py| {
            let msg = self
                .0
                .value(py)
                .str()
                .and_then(|s| s.extract::<String>())
                .unwrap_or_else(|_| "Python error".to_string());
            write!(f, "{msg}")
        })
    }
}

impl std::error::Error for WrappedPyErr {}

// SAFETY: PyErr is Send. We implement Sync because WrappedPyErr is only
// accessed while holding the GIL (via Python::attach), which provides
// the necessary synchronization.
unsafe impl Sync for WrappedPyErr {}

fn pyerr_to_io(err: &PyErr) -> io::Error {
    Python::attach(|py| io::Error::other(WrappedPyErr(err.clone_ref(py))))
}

fn pyerr_from_io(err: &io::Error) -> Option<PyErr> {
    err.get_ref()?
        .downcast_ref::<WrappedPyErr>()
        .map(|w| Python::attach(|py| w.0.clone_ref(py)))
}

fn is_generator(py: Python, xml_input: &Bound<'_, PyAny>) -> PyResult<bool> {
    let types = PyModule::import(py, "types")?;
    let generator_type = types.getattr("GeneratorType")?;
    xml_input.is_instance(&generator_type)
}

#[derive(Default)]
struct PendingBytes {
    buf: Vec<u8>,
    offset: usize,
}

impl PendingBytes {
    fn remaining(&self) -> usize {
        self.buf.len().saturating_sub(self.offset)
    }

    fn is_empty(&self) -> bool {
        self.remaining() == 0
    }

    fn clear(&mut self) {
        self.buf.clear();
        self.offset = 0;
    }

    fn fill_from_slice(&mut self, bytes: &[u8]) {
        self.buf.clear();
        self.buf.extend_from_slice(bytes);
        self.offset = 0;
    }

    fn copy_into(&mut self, out: &mut [u8]) -> usize {
        let Some(remaining) = self.buf.get(self.offset..) else {
            self.clear();
            return 0;
        };

        let to_copy = remaining.len().min(out.len());
        let Some(dst) = out.get_mut(..to_copy) else {
            return 0;
        };
        let Some(src) = remaining.get(..to_copy) else {
            return 0;
        };
        dst.copy_from_slice(src);
        self.offset = self.offset.saturating_add(to_copy);
        if self.offset >= self.buf.len() {
            self.clear();
        }
        to_copy
    }
}

struct PyFileLikeRead {
    file_like: Py<PyAny>,
    pending: PendingBytes,
    bytearray_buffer: Option<Vec<u8>>,
}

impl PyFileLikeRead {
    fn new(file_like: Py<PyAny>) -> Self {
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

            let bytes = if let Ok(chunk_bytes) = chunk.downcast::<PyBytes>() {
                chunk_bytes.as_bytes()
            } else if let Ok(chunk_bytearray) = chunk.downcast::<PyByteArray>() {
                self.bytearray_buffer = Some(chunk_bytearray.to_vec());
                if let Some(bytes_ref) = self.bytearray_buffer.as_deref() {
                    bytes_ref
                } else {
                    return Err(pyerr_to_io(
                        &PyErr::new::<pyo3::exceptions::PyTypeError, _>(
                            "read() did not return a bytes object (type=bytearray)",
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
                        "read() did not return a bytes object (type={type_name})"
                    )),
                ));
            };

            if bytes.is_empty() {
                return Ok(0);
            }

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

struct PyGeneratorRead {
    generator: Py<PyAny>,
    pending: PendingBytes,
    done: bool,
    bytearray_buffer: Option<Vec<u8>>,
}

impl PyGeneratorRead {
    fn new(generator: Py<PyAny>) -> Self {
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

fn extract_hashmap(py: Python, dict_input: &Py<PyAny>) -> PyResult<HashMap<String, String>> {
    let dict = dict_input.downcast_bound::<PyDict>(py).map_err(|_| {
        PyErr::new::<pyo3::exceptions::PyTypeError, _>("namespaces must be a dictionary")
    })?;

    let mut hashmap = HashMap::with_capacity(dict.len());

    for (key, value) in dict {
        let key_str = key.downcast::<PyString>().map_err(|_| {
            PyErr::new::<pyo3::exceptions::PyTypeError, _>("namespace keys must be strings")
        })?;

        let value_str = value.downcast::<PyString>().map_err(|_| {
            PyErr::new::<pyo3::exceptions::PyTypeError, _>("namespace values must be strings")
        })?;

        hashmap.insert(key_str.to_string(), value_str.to_string());
    }

    Ok(hashmap)
}

fn expat_error(py: Python, msg: String) -> PyErr {
    let expat_type = PyModule::import(py, "xml.parsers.expat")
        .and_then(|m| m.getattr("ExpatError"))
        .ok()
        .and_then(|t| t.downcast_into::<PyType>().ok());
    match expat_type {
        Some(ty) => PyErr::from_type(ty, msg),
        None => PyErr::new::<pyo3::exceptions::PyValueError, _>(format!("XML parse error: {msg}")),
    }
}

fn validate_element_name(py: Python, name: &str) -> PyResult<()> {
    if name.is_empty() || name.chars().any(|x| matches!(x, '<' | '>')) {
        return Err(expat_error(
            py,
            "not well-formed (invalid element name)".to_string(),
        ));
    }
    Ok(())
}

fn map_quick_xml_error(py: Python, err: quick_xml::Error) -> PyErr {
    match err {
        quick_xml::Error::Io(io_err) => {
            pyerr_from_io(&io_err).unwrap_or_else(|| expat_error(py, io_err.to_string()))
        }
        other @ (quick_xml::Error::NonDecodable(_)
        | quick_xml::Error::UnexpectedEof(_)
        | quick_xml::Error::EndEventMismatch { .. }
        | quick_xml::Error::UnexpectedToken(_)
        | quick_xml::Error::UnexpectedBang(_)
        | quick_xml::Error::TextNotFound
        | quick_xml::Error::XmlDeclWithoutVersion(_)
        | quick_xml::Error::EmptyDocType
        | quick_xml::Error::InvalidAttr(_)
        | quick_xml::Error::EscapeError(_)
        | quick_xml::Error::UnknownPrefix(_)
        | quick_xml::Error::InvalidPrefixBind { .. }) => expat_error(py, other.to_string()),
    }
}

fn parse_xml_with_reader<R: BufRead>(
    py: Python,
    reader: R,
    config: &ParseConfig,
    force_list: Option<Py<PyAny>>,
    postprocessor: Option<Py<PyAny>>,
    strip_whitespace: bool,
    process_comments: bool,
) -> PyResult<Py<PyAny>> {
    let mut parser = XmlParser::new(config.clone(), force_list, postprocessor);
    let mut xml_reader = Reader::from_reader(reader);
    xml_reader
        .trim_text(strip_whitespace)
        .check_end_names(true)
        .check_comments(true)
        .expand_empty_elements(true);

    let mut buf = Vec::with_capacity(128);

    loop {
        match xml_reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = std::str::from_utf8(e.name().into_inner())?;
                validate_element_name(py, name)?;
                let attrs: Vec<_> = e
                    .attributes()
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| expat_error(py, e.to_string()))?;
                parser.start_element(py, name, &attrs)?;
            }
            Ok(Event::End(ref e)) => {
                let name = std::str::from_utf8(e.name().into_inner())?;
                validate_element_name(py, name)?;
                parser.end_element(py, name)?;
            }
            Ok(Event::Empty(ref e)) => {
                let name = std::str::from_utf8(e.name().into_inner())?;
                validate_element_name(py, name)?;

                let attrs: Vec<_> = e
                    .attributes()
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| expat_error(py, e.to_string()))?;
                parser.start_element(py, name, &attrs)?;
                parser.end_element(py, name)?;
            }
            Ok(Event::Text(ref e)) => {
                let text = e.unescape().map_err(|e| expat_error(py, e.to_string()))?;
                parser.characters(&text);
            }
            Ok(Event::CData(ref e)) => {
                parser.characters(std::str::from_utf8(e.as_ref())?);
            }
            Ok(Event::Comment(ref e)) if process_comments => {
                parser.comment(py, std::str::from_utf8(e.as_ref())?)?;
            }
            Ok(Event::Eof) => {
                break;
            }
            Err(e) => return Err(map_quick_xml_error(py, e)),
            _ => {}
        }
        buf.clear();
    }

    if !parser.path.is_empty()
        || !parser.text_stack.is_empty()
        || !parser.namespace_stack.is_empty()
    {
        return Err(expat_error(py, "unclosed element(s) found".to_string()));
    }

    match parser.stack.as_slice() {
        [one] => Ok(one.clone_ref(py)),
        [] => Err(expat_error(py, "no element found".to_string())),
        [_, ..] => Err(expat_error(py, "unclosed element(s) found".to_string())),
    }
}

/// Parse XML string/bytes into a Python dictionary
#[allow(clippy::too_many_arguments)]
#[allow(clippy::fn_params_excessive_bools)]
#[pyfunction]
#[pyo3(signature = (
    xml_input,
    _encoding = None,
    process_namespaces = false,
    namespace_separator = ":",
    disable_entities = true,
    process_comments = false,
    xml_attribs = true,
    attr_prefix = "@",
    cdata_key = "#text",
    force_cdata = false,
    cdata_separator = "",
    strip_whitespace = true,
    force_list = None,
    postprocessor = None,
    item_depth = 0,
    comment_key = "#comment",
    namespaces = None,
))]
fn parse(
    py: Python,
    xml_input: &Bound<'_, PyAny>,
    _encoding: Option<&str>,
    process_namespaces: bool,
    namespace_separator: &str,
    disable_entities: bool,
    process_comments: bool,
    xml_attribs: bool,
    attr_prefix: &str,
    cdata_key: &str,
    force_cdata: bool,
    cdata_separator: &str,
    strip_whitespace: bool,
    force_list: Option<Py<PyAny>>,
    postprocessor: Option<Py<PyAny>>,
    item_depth: usize,
    comment_key: &str,
    namespaces: Option<Py<PyAny>>,
) -> PyResult<Py<PyAny>> {
    let namespaces_rs = namespaces
        .map(|dict_py| extract_hashmap(py, &dict_py))
        .transpose()?;

    let config = ParseConfig {
        xml_attribs,
        attr_prefix: attr_prefix.to_string(),
        cdata_key: cdata_key.to_string(),
        force_cdata,
        cdata_separator: cdata_separator.to_string(),
        strip_whitespace,
        namespace_separator: namespace_separator.to_string(),
        process_namespaces,
        process_comments,
        comment_key: comment_key.to_string(),
        item_depth,
        disable_entities,
        namespaces: namespaces_rs,
    };

    if let Ok(xml_str) = xml_input.downcast::<PyString>() {
        let text = xml_str.to_str()?;
        return parse_xml_with_reader(
            py,
            text.as_bytes(),
            &config,
            force_list,
            postprocessor,
            strip_whitespace,
            process_comments,
        );
    }

    if let Ok(xml_bytes) = xml_input.downcast::<PyBytes>() {
        return parse_xml_with_reader(
            py,
            xml_bytes.as_bytes(),
            &config,
            force_list,
            postprocessor,
            strip_whitespace,
            process_comments,
        );
    }

    if let Ok(read_attr) = xml_input.getattr("read") {
        if read_attr.is_callable() {
            let reader = BufReader::new(PyFileLikeRead::new(xml_input.clone().unbind()));
            return parse_xml_with_reader(
                py,
                reader,
                &config,
                force_list,
                postprocessor,
                strip_whitespace,
                process_comments,
            );
        }
    }

    if is_generator(py, xml_input)? {
        let reader = BufReader::new(PyGeneratorRead::new(xml_input.clone().unbind()));
        return parse_xml_with_reader(
            py,
            reader,
            &config,
            force_list,
            postprocessor,
            strip_whitespace,
            process_comments,
        );
    }

    let xml_bytes = xml_input.extract::<&[u8]>()?;
    parse_xml_with_reader(
        py,
        xml_bytes,
        &config,
        force_list,
        postprocessor,
        strip_whitespace,
        process_comments,
    )
}

struct UnparseConfig {
    encoding: String,
    full_document: bool,
    short_empty_elements: bool,
    attr_prefix: String,
    cdata_key: String,
    pretty: bool,
    newl: String,
    indent: String,
}

struct XmlWriter {
    config: UnparseConfig,
    indent_level: usize,
    output: String,
    preprocessor: Option<Py<PyAny>>,
}

impl XmlWriter {
    fn new(config: UnparseConfig, preprocessor: Option<Py<PyAny>>) -> Self {
        Self {
            config,
            indent_level: 0,
            output: String::new(),
            preprocessor,
        }
    }

    fn write_header(&mut self) {
        if self.config.full_document {
            self.output.push_str(r#"<?xml version="1.0" encoding=""#);
            self.output.push_str(&self.config.encoding);
            self.output.push_str(r#""?>"#);
            self.output.push_str(&self.config.newl);
        }
    }

    fn write_indent(&mut self) {
        if self.config.pretty {
            for _ in 0..self.indent_level {
                self.output.push_str(&self.config.indent);
            }
        }
    }

    #[inline]
    fn apply_preprocessor<'py>(
        &self,
        py: Python<'py>,
        key: &str,
        data: &Bound<'py, PyAny>,
    ) -> PyResult<Option<(String, Bound<'py, PyAny>)>> {
        let mut final_key = key.to_string();
        let mut final_value = data.clone();

        if let Some(proc) = &self.preprocessor {
            let result = proc.call1(py, (key, data))?;

            if result.is_none(py) {
                return Ok(None);
            }

            let tuple = result.bind(py).downcast::<PyTuple>()?;
            final_key = tuple.get_item(0)?.extract::<String>()?;
            final_value = tuple.get_item(1)?;
        }

        Ok(Some((final_key, final_value)))
    }

    fn write_element(
        &mut self,
        py: Python,
        tag: &str,
        value: &Bound<'_, PyAny>,
        needs_newline: bool,
    ) -> PyResult<()> {
        let Some((final_tag, final_value)) = self.apply_preprocessor(py, tag, value)? else {
            return Ok(());
        };

        if self.config.pretty && needs_newline {
            self.output.push_str(&self.config.newl);
            self.write_indent();
        }

        // Check if value is None (empty element)
        if final_value.is_none() {
            if self.config.short_empty_elements {
                self.output.push('<');
                self.output.push_str(final_tag.as_str());
                self.output.push_str("/>");
            } else {
                self.output.push('<');
                self.output.push_str(final_tag.as_str());
                self.output.push_str("></");
                self.output.push_str(final_tag.as_str());
                self.output.push('>');
            }
            return Ok(());
        }

        // Check if value is a dict (element with attributes/children)
        if let Ok(dict) = final_value.downcast::<PyDict>() {
            self.write_dict_element(py, final_tag.as_str(), dict)?;
        } else if let Ok(list) = final_value.downcast::<PyList>() {
            for (i, item) in list.iter().enumerate() {
                self.write_element(py, final_tag.as_str(), &item, i > 0 || needs_newline)?;
            }
        } else if let Ok(bool_val) = final_value.extract::<bool>() {
            let bool_text = if bool_val { "true" } else { "false" };
            self.output.push('<');
            self.output.push_str(final_tag.as_str());
            self.output.push('>');
            self.output.push_str(bool_text);
            self.output.push_str("</");
            self.output.push_str(final_tag.as_str());
            self.output.push('>');
        } else {
            let val = final_value.str()?.to_string();
            self.output.push('<');
            self.output.push_str(final_tag.as_str());
            self.output.push('>');
            self.output.push_str(escape_xml(&val).as_ref());
            self.output.push_str("</");
            self.output.push_str(final_tag.as_str());
            self.output.push('>');
        }

        Ok(())
    }

    fn write_dict_element(
        &mut self,
        py: Python,
        tag: &str,
        dict: &Bound<'_, PyDict>,
    ) -> PyResult<()> {
        let mut attributes = Vec::new();
        let mut text_content = None;
        let mut child_elements = Vec::new();

        for (key, value) in dict {
            let key_str = key.str()?.to_string();

            if let Some(attr_name) = key_str.strip_prefix(&self.config.attr_prefix) {
                let attr_value = if let Ok(bool_val) = value.extract::<bool>() {
                    if bool_val {
                        "true".to_string()
                    } else {
                        "false".to_string()
                    }
                } else {
                    value.str()?.to_string()
                };
                attributes.push((attr_name.to_string(), attr_value));
            } else if key_str == self.config.cdata_key {
                let text = if let Ok(bool_val) = value.extract::<bool>() {
                    if bool_val {
                        "true".to_string()
                    } else {
                        "false".to_string()
                    }
                } else {
                    value.str()?.to_string()
                };
                text_content = Some(text);
            } else {
                child_elements.push((key_str, value));
            }
        }

        self.output.push('<');
        self.output.push_str(tag);
        for (attr_name, attr_value) in attributes {
            self.output.push(' ');
            self.output.push_str(&attr_name);
            self.output.push_str("=\"");
            self.output.push_str(escape_xml_attr(&attr_value).as_ref());
            self.output.push('"');
        }

        if child_elements.is_empty() && text_content.is_none() {
            if self.config.short_empty_elements {
                self.output.push_str("/>");
            } else {
                self.output.push_str("></");
                self.output.push_str(tag);
                self.output.push('>');
            }
        } else {
            self.output.push('>');

            if let Some(text) = text_content {
                self.output.push_str(&escape_xml(&text));
            }

            if !child_elements.is_empty() {
                self.indent_level += 1;
                for (i, (child_tag, child_value)) in child_elements.into_iter().enumerate() {
                    self.write_element(py, &child_tag, &child_value, i > 0 || self.config.pretty)?;
                }
                self.indent_level -= 1;

                if self.config.pretty {
                    self.output.push_str(&self.config.newl);
                    self.write_indent();
                }
            }

            self.output.push_str("</");
            self.output.push_str(tag);
            self.output.push('>');
        }

        Ok(())
    }

    fn finish(self) -> String {
        self.output
    }
}

const LT: u8 = b'<';
const GT: u8 = b'>';
const AMPERSAND: u8 = b'&';

const ESCAPED_AMP: &str = "&amp;";
const ESCAPED_LT: &str = "&lt;";
const ESCAPED_GT: &str = "&gt;";

fn escape_xml(text: &str) -> Cow<'_, str> {
    let bytes = text.as_bytes();
    let len = bytes.len();

    let need_escape = memchr::memchr(AMPERSAND, bytes).is_some()
        || memchr::memchr(LT, bytes).is_some()
        || memchr::memchr(GT, bytes).is_some();

    if !need_escape {
        return Cow::Borrowed(text);
    }

    let mut i = 0;
    let mut last_pos = 0;
    let mut result = String::with_capacity(len * 6);

    let ptr = bytes.as_ptr();

    while i < len {
        let byte = unsafe {
            // SAFETY: `ptr` comes from `bytes.as_ptr()` which is valid for reads,
            // and `i` is bounded by `bytes.len()`, so `ptr.add(i)` is within bounds.
            *ptr.add(i)
        };
        match byte {
            AMPERSAND | LT | GT => {
                if last_pos < i {
                    let slice = unsafe {
                        // SAFETY: The slice from `last_pos` to `i` is valid UTF-8 because
                        // it's a subslice of the original `text` which is guaranteed to be valid UTF-8.
                        from_utf8_unchecked(from_raw_parts(ptr.add(last_pos), i - last_pos))
                    };
                    result.push_str(slice);
                }

                match byte {
                    AMPERSAND => result.push_str(ESCAPED_AMP),
                    LT => result.push_str(ESCAPED_LT),
                    GT => result.push_str(ESCAPED_GT),
                    _ => unreachable!(),
                }
                last_pos = i + 1;
            }
            _ => {}
        }
        i += 1;
    }

    if last_pos < len {
        let slice = unsafe {
            // SAFETY: The slice from `last_pos` to `bytes.len()` is valid UTF-8 because
            // it's a subslice of the original `text` which is guaranteed to be valid UTF-8.
            from_utf8_unchecked(from_raw_parts(ptr.add(last_pos), len - last_pos))
        };
        result.push_str(slice);
    }

    Cow::Owned(result)
}

fn escape_xml_attr(text: &str) -> Cow<'_, str> {
    let mut result: Option<String> = None;
    let mut last_pos = 0;

    for (i, ch) in text.char_indices() {
        match ch {
            '&' | '<' | '>' | '"' => {
                let is_first_escape = result.is_none();
                let s = result.get_or_insert_with(|| {
                    let mut output = String::with_capacity(text.len() + 20);
                    output.push_str(&text[..i]);
                    output
                });
                if !is_first_escape {
                    s.push_str(&text[last_pos..i]);
                }
                match ch {
                    '&' => s.push_str("&amp;"),
                    '<' => s.push_str("&lt;"),
                    '>' => s.push_str("&gt;"),
                    '"' => s.push_str("&quot;"),
                    _ => unreachable!(),
                }
                last_pos = i + ch.len_utf8();
            }
            _ => {}
        }
    }

    match result {
        None => Cow::Borrowed(text),
        Some(mut s) => {
            if last_pos < text.len() {
                s.push_str(&text[last_pos..]);
            }
            Cow::Owned(s)
        }
    }
}

/// Convert Python dictionary back to XML string
#[allow(clippy::too_many_arguments)]
#[pyfunction]
#[pyo3(signature = (
    input_dict,
    _output = None,
    encoding = "utf-8",
    full_document = true,
    short_empty_elements = false,
    attr_prefix = "@",
    cdata_key = "#text",
    pretty = false,
    newl = "\n",
    indent = "\t",
    preprocessor = None
))]
fn unparse(
    py: Python,
    input_dict: &Bound<'_, PyDict>,
    _output: Option<&Bound<'_, PyAny>>,
    encoding: &str,
    full_document: bool,
    short_empty_elements: bool,
    attr_prefix: &str,
    cdata_key: &str,
    pretty: bool,
    newl: &str,
    indent: &str,
    preprocessor: Option<Py<PyAny>>,
) -> PyResult<Py<PyAny>> {
    let config = UnparseConfig {
        encoding: encoding.to_string(),
        full_document,
        short_empty_elements,
        attr_prefix: attr_prefix.to_string(),
        cdata_key: cdata_key.to_string(),
        pretty,
        newl: newl.to_string(),
        indent: indent.to_string(),
    };

    let mut writer = XmlWriter::new(config, preprocessor);

    // Validate root elements
    let dict_len = input_dict.len();

    if full_document {
        if dict_len == 0 {
            return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                "Document must have exactly one root",
            ));
        }
        if dict_len > 1 {
            return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                "Document must have exactly one root",
            ));
        }
    }

    writer.write_header();

    // Write elements
    for (i, (key, value)) in input_dict.iter().enumerate() {
        let tag = key.str()?.to_string();
        writer.write_element(py, &tag, &value, i > 0)?;
    }

    let result = writer.finish();
    Ok(result.into_pyobject(py)?.into_any().unbind())
}

/// A Python module implemented in Rust.
#[pymodule]
fn xmltodict_rs(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(parse, m)?)?;
    m.add_function(wrap_pyfunction!(unparse, m)?)?;
    m.add("__version__", "0.1.0")?;
    m.add("__build_id__", "v2-2024-08-15")?;
    Ok(())
}

#[test]
fn test_escape_xml() {
    assert_eq!(
        "Start &amp; then &lt; some &gt; text &amp; more &lt; text &gt; end",
        escape_xml("Start & then < some > text & more < text > end")
    );
}
