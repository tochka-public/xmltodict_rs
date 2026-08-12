#[cfg(all(
    feature = "mimalloc",
    any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "windows", target_arch = "x86_64"),
        target_os = "macos"
    )
))]
use mimalloc::MiMalloc;

mod config;
mod error;
mod escape;
mod parser;
mod reader;
mod unparser;

use config::{AttrPrefix, CdataKey, CommentKey, NamespaceSeparator, ParseConfig, UnparseConfig};
use error::{expat_error, map_quick_xml_error, validate_element_name};
use parser::XmlParser;
use reader::{PyFileLikeRead, PyGeneratorRead};
use unparser::XmlWriter;

use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyModule, PyString};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};

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

/// Decode UTF-8 bytes, mapping invalid sequences to `UnicodeDecodeError`
/// (matches the automatic `From<std::str::Utf8Error>` conversion that pyo3
/// provided before 0.29 removed it).
fn utf8_str(bytes: &[u8]) -> PyResult<&str> {
    std::str::from_utf8(bytes)
        .map_err(|e| pyo3::exceptions::PyUnicodeDecodeError::new_err(e.to_string()))
}

/// Resolve a `&entity;` / `&#NN;` general reference (quick-xml 0.41 reports
/// these as standalone events instead of embedding them in `Event::Text`).
/// Only character references and the five predefined XML entities are
/// resolved -- DTD-declared custom entities stay unsupported, matching
/// `disable_entities=False` being unimplemented.
fn resolve_general_ref(py: Python, r: &quick_xml::events::BytesRef) -> PyResult<String> {
    if let Some(ch) = r
        .resolve_char_ref()
        .map_err(|e| map_quick_xml_error(py, e))?
    {
        return Ok(ch.to_string());
    }
    let name = r.decode().map_err(|e| expat_error(py, e.to_string()))?;
    quick_xml::escape::resolve_predefined_entity(&name)
        .map(str::to_owned)
        .ok_or_else(|| expat_error(py, format!("undefined entity &{name};")))
}

fn is_generator(py: Python, xml_input: &Bound<'_, PyAny>) -> PyResult<bool> {
    let types = PyModule::import(py, "types")?;
    let generator_type = types.getattr("GeneratorType")?;
    xml_input.is_instance(&generator_type)
}

fn extract_hashmap(py: Python, dict_input: &Py<PyAny>) -> PyResult<HashMap<String, String>> {
    let dict = dict_input.cast_bound::<PyDict>(py).map_err(|_err| {
        PyErr::new::<pyo3::exceptions::PyTypeError, _>("namespaces must be a dictionary")
    })?;

    let mut hashmap = HashMap::with_capacity(dict.len());

    for (key, value) in dict {
        let key_str = key.cast::<PyString>().map_err(|_err| {
            PyErr::new::<pyo3::exceptions::PyTypeError, _>("namespace keys must be strings")
        })?;

        let value_str = value.cast::<PyString>().map_err(|_err| {
            PyErr::new::<pyo3::exceptions::PyTypeError, _>("namespace values must be strings")
        })?;

        hashmap.insert(key_str.to_string(), value_str.to_string());
    }

    Ok(hashmap)
}

fn parse_xml_with_reader<R: BufRead>(
    py: Python,
    reader: R,
    config: &ParseConfig,
    force_list: Option<Py<PyAny>>,
    postprocessor: Option<Py<PyAny>>,
    process_comments: bool,
) -> PyResult<Py<PyAny>> {
    let mut parser = XmlParser::new(config.clone(), force_list, postprocessor);
    let mut xml_reader = Reader::from_reader(reader);
    let reader_config = xml_reader.config_mut();
    // Whitespace stripping is handled ourselves in `XmlParser::end_element` on the
    // fully joined text of an element, not per-event here: quick-xml 0.41 reports
    // `&entity;`/`&#NN;` references as standalone `Event::GeneralRef` events, so a
    // text run like "a &amp; b" now arrives as three events ("a ", GeneralRef, " b")
    // instead of one. Trimming each event individually (the pre-0.41 approach) would
    // eat the whitespace adjacent to every entity instead of just the run's edges.
    reader_config.check_end_names = true;
    reader_config.check_comments = true;
    reader_config.expand_empty_elements = true;

    let mut buf = Vec::with_capacity(128);
    let mut root_closed = false;

    loop {
        match xml_reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                if root_closed {
                    return Err(expat_error(py, "junk after document element".to_owned()));
                }
                let name = utf8_str(e.name().into_inner())?;
                validate_element_name(py, name)?;
                let attrs: Vec<_> = e
                    .attributes()
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| expat_error(py, e.to_string()))?;
                parser.start_element(py, name, &attrs)?;
            }
            Ok(Event::End(ref e)) => {
                let name = utf8_str(e.name().into_inner())?;
                validate_element_name(py, name)?;
                parser.end_element(py, name)?;
                if parser.path.is_empty() {
                    root_closed = true;
                }
            }
            Ok(Event::Empty(ref e)) => {
                if root_closed {
                    return Err(expat_error(py, "junk after document element".to_owned()));
                }
                let name = utf8_str(e.name().into_inner())?;
                validate_element_name(py, name)?;

                let attrs: Vec<_> = e
                    .attributes()
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| expat_error(py, e.to_string()))?;
                parser.start_element(py, name, &attrs)?;
                parser.end_element(py, name)?;
                if parser.path.is_empty() {
                    root_closed = true;
                }
            }
            Ok(Event::Text(ref e)) => {
                let text = e.decode().map_err(|e| expat_error(py, e.to_string()))?;
                if parser.path.is_empty() && !text.trim().is_empty() {
                    let msg = if root_closed {
                        "junk after document element"
                    } else {
                        "syntax error"
                    };
                    return Err(expat_error(py, msg.to_owned()));
                }
                parser.characters(&text);
            }
            Ok(Event::GeneralRef(ref e)) => {
                if parser.path.is_empty() {
                    let msg = if root_closed {
                        "junk after document element"
                    } else {
                        "syntax error"
                    };
                    return Err(expat_error(py, msg.to_owned()));
                }
                let resolved = resolve_general_ref(py, e)?;
                parser.characters(&resolved);
            }
            Ok(Event::CData(ref e)) => {
                if parser.path.is_empty() {
                    return Err(expat_error(py, "junk after document element".to_owned()));
                }
                parser.characters(utf8_str(e.as_ref())?);
            }
            Ok(Event::Comment(ref e)) if process_comments => {
                parser.comment(py, utf8_str(e.as_ref())?)?;
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
        return Err(expat_error(py, "unclosed element(s) found".to_owned()));
    }

    match parser.stack.as_slice() {
        [one] => Ok(one.clone_ref(py)),
        [] => Err(expat_error(py, "no element found".to_owned())),
        [_, ..] => Err(expat_error(py, "unclosed element(s) found".to_owned())),
    }
}

/// Parse XML string/bytes into a Python dictionary
#[allow(clippy::too_many_arguments)]
#[allow(clippy::fn_params_excessive_bools)]
#[pyfunction]
#[pyo3(signature = (
    xml_input,
    encoding = None,
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
    item_callback = None,
    comment_key = "#comment",
    namespaces = None,
))]
fn parse(
    py: Python,
    xml_input: &Bound<'_, PyAny>,
    encoding: Option<&str>,
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
    item_callback: Option<&Bound<'_, PyAny>>,
    comment_key: &str,
    namespaces: Option<Py<PyAny>>,
) -> PyResult<Py<PyAny>> {
    if item_depth > 0 || item_callback.is_some() {
        return Err(PyErr::new::<pyo3::exceptions::PyNotImplementedError, _>(
            "streaming mode (item_depth/item_callback) is not implemented in xmltodict_rs",
        ));
    }
    if !disable_entities {
        return Err(PyErr::new::<pyo3::exceptions::PyNotImplementedError, _>(
            "disable_entities=False (DTD entity expansion) is not implemented in xmltodict_rs",
        ));
    }
    if let Some(enc) = encoding {
        if !enc.eq_ignore_ascii_case("utf-8") && !enc.eq_ignore_ascii_case("utf8") {
            return Err(PyErr::new::<pyo3::exceptions::PyNotImplementedError, _>(
                format!("encoding '{enc}' is not supported, only UTF-8"),
            ));
        }
    }

    let namespaces_rs = namespaces
        .map(|dict_py| extract_hashmap(py, &dict_py))
        .transpose()?;

    let config = ParseConfig {
        xml_attribs,
        attr_prefix: AttrPrefix::new(attr_prefix),
        cdata_key: CdataKey::new(cdata_key),
        force_cdata,
        cdata_separator: cdata_separator.to_owned(),
        strip_whitespace,
        namespace_separator: NamespaceSeparator::new(namespace_separator),
        process_namespaces,
        comment_key: CommentKey::new(comment_key),
        namespaces: namespaces_rs,
    };

    if let Ok(xml_str) = xml_input.cast::<PyString>() {
        let text = xml_str.to_str()?;
        return parse_xml_with_reader(
            py,
            text.as_bytes(),
            &config,
            force_list,
            postprocessor,
            process_comments,
        );
    }

    if let Ok(xml_bytes) = xml_input.cast::<PyBytes>() {
        return parse_xml_with_reader(
            py,
            xml_bytes.as_bytes(),
            &config,
            force_list,
            postprocessor,
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
        process_comments,
    )
}

/// Convert Python dictionary back to XML string
#[allow(clippy::too_many_arguments)]
#[pyfunction]
#[pyo3(signature = (
    input_dict,
    output = None,
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
    output: Option<&Bound<'_, PyAny>>,
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
        encoding: encoding.to_owned(),
        full_document,
        short_empty_elements,
        attr_prefix: AttrPrefix::new(attr_prefix),
        cdata_key: CdataKey::new(cdata_key),
        pretty,
        newl: newl.to_owned(),
        indent: indent.to_owned(),
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

    if let Some(out) = output {
        out.call_method1("write", (result,))?;
        return Ok(py.None());
    }

    Ok(result.into_pyobject(py)?.into_any().unbind())
}

#[pymodule(gil_used = false)]
fn xmltodict_rs(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(parse, m)?)?;
    m.add_function(wrap_pyfunction!(unparse, m)?)?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
