use pyo3::prelude::*;
use pyo3::types::{PyModule, PyType};
use std::io;

/// Wrapper to store `PyErr` inside `io::Error` while preserving the original exception type.
/// `PyErr` is Send but not Sync, so we need unsafe impl Sync.
/// This is safe because we only access the inner `PyErr` while holding the GIL.
pub struct WrappedPyErr(pub PyErr);

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

pub fn pyerr_to_io(err: &PyErr) -> io::Error {
    Python::attach(|py| io::Error::other(WrappedPyErr(err.clone_ref(py))))
}

pub fn pyerr_from_io(err: &io::Error) -> Option<PyErr> {
    err.get_ref()?
        .downcast_ref::<WrappedPyErr>()
        .map(|w| Python::attach(|py| w.0.clone_ref(py)))
}

pub fn expat_error(py: Python, msg: String) -> PyErr {
    let expat_type = PyModule::import(py, "xml.parsers.expat")
        .and_then(|m| m.getattr("ExpatError"))
        .ok()
        .and_then(|t| t.downcast_into::<PyType>().ok());
    match expat_type {
        Some(ty) => PyErr::from_type(ty, msg),
        None => PyErr::new::<pyo3::exceptions::PyValueError, _>(format!("XML parse error: {msg}")),
    }
}

pub fn validate_element_name(py: Python, name: &str) -> PyResult<()> {
    if name.is_empty() || name.chars().any(|x| matches!(x, '<' | '>')) {
        return Err(expat_error(
            py,
            "not well-formed (invalid element name)".to_string(),
        ));
    }
    Ok(())
}

pub fn map_quick_xml_error(py: Python, err: quick_xml::Error) -> PyErr {
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
