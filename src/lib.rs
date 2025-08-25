use pyo3::prelude::*;
use pyo3::types::{PyAny, PyBytes, PyDict, PyList, PyModule, PyString};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::borrow::Cow;
use std::fmt::Write;

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
        }
    }
}

pub struct XmlParser {
    config: ParseConfig,
    force_list: Option<PyObject>,
    stack: Vec<PyObject>,
    path: Vec<String>,
    text_stack: Vec<Vec<String>>,
}

impl XmlParser {
    #[must_use]
    pub fn new(config: ParseConfig, force_list: Option<PyObject>) -> Self {
        Self {
            config,
            force_list,
            stack: Vec::new(),
            path: Vec::new(),
            text_stack: Vec::new(),
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

    fn push_data(
        &self,
        py: Python,
        item: &Bound<'_, PyDict>,
        key: &str,
        data: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        if item.contains(key)? {
            // Key exists - convert to list or extend list
            let existing = item.get_item(key)?.unwrap();
            if let Ok(list) = existing.downcast::<PyList>() {
                list.append(data.clone())?;
            } else {
                let new_list = PyList::new(py, [existing.clone(), data.clone()])?;
                item.set_item(key, &new_list)?;
            }
        } else {
            // Key doesn't exist - check force_list
            if self.should_force_list(py, key, data)? {
                let new_list = PyList::new(py, [data.clone()])?;
                item.set_item(key, &new_list)?;
            } else {
                item.set_item(key, data)?;
            }
        }
        Ok(())
    }

    fn build_name(&self, full_name: &str) -> String {
        if !self.config.process_namespaces {
            return full_name.to_string();
        }

        if let Some(sep_pos) = full_name.rfind(&self.config.namespace_separator) {
            let (namespace, name) = full_name.split_at(sep_pos);
            let name = &name[self.config.namespace_separator.len()..];

            // TODO: implement namespace resolution logic
            return format!("{namespace}:{name}");
        }

        full_name.to_string()
    }

    fn start_element(
        &mut self,
        py: Python,
        name: &str,
        attrs: &[quick_xml::events::attributes::Attribute],
    ) -> PyResult<()> {
        let element_name = self.build_name(name);
        let element_dict = PyDict::new(py);
        if self.config.xml_attribs && !attrs.is_empty() {
            for attr in attrs {
                let key = std::str::from_utf8(attr.key.as_ref())?;
                let value = std::str::from_utf8(attr.value.as_ref())?;
                let prefixed_key = format!("{}{}", self.config.attr_prefix, self.build_name(key));
                element_dict.set_item(prefixed_key, value)?;
            }
        }

        self.stack.push(element_dict.into());
        self.path.push(element_name);
        self.text_stack.push(Vec::new());
        Ok(())
    }

    fn end_element(&mut self, py: Python, name: &str) -> PyResult<()> {
        let element_name = self.build_name(name);

        // Get current element and text
        let current_element = self.stack.pop().unwrap();
        let text_parts = self.text_stack.pop().unwrap();
        self.path.pop();

        // Get text content
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

        // Build element value
        let element_dict = current_element.downcast_bound::<PyDict>(py)?;
        let has_attrs = !element_dict.is_empty();
        let has_text = text_content.is_some();

        let final_value = if !has_attrs && !has_text {
            // Empty element
            py.None()
        } else if !has_attrs && has_text {
            // Only text
            let text = text_content.unwrap();
            if self.config.force_cdata {
                let dict = PyDict::new(py);
                dict.set_item(&self.config.cdata_key, text)?;
                dict.into()
            } else {
                text.into_pyobject(py).unwrap().into_any().unbind()
            }
        } else if has_text {
            // Attributes + text
            element_dict.set_item(&self.config.cdata_key, text_content.unwrap())?;
            current_element
        } else {
            // Only attributes
            current_element
        };

        if self.stack.is_empty() {
            // Root element - create final result
            let result_dict = PyDict::new(py);
            result_dict.set_item(element_name, final_value)?;
            self.stack.push(result_dict.into());
        } else {
            // Add to parent
            let parent = self.stack.last().unwrap();
            let parent_dict = parent.downcast_bound::<PyDict>(py)?;

            self.push_data(py, parent_dict, &element_name, final_value.bind(py))?;
        }

        Ok(())
    }

    fn characters(&mut self, data: &str) {
        if let Some(current_text) = self.text_stack.last_mut() {
            current_text.push(data.to_string());
        }
    }

    fn comment(&self, py: Python, comment: &str) -> PyResult<()> {
        let Some(parent) = self.stack.last() else { return Ok(()); };
        let parent_dict = parent.downcast_bound::<PyDict>(py)?;
        let comment_py = comment.trim().into_pyobject(py)?;
        self.push_data(py, parent_dict, &self.config.comment_key, &comment_py)
    }
}

fn extract_xml_bytes(xml_input: &Bound<'_, PyAny>) -> PyResult<Vec<u8>> {
    if let Ok(s) = xml_input.downcast::<PyString>() {
        Ok(s.to_string().into_bytes())
    } else if let Ok(b) = xml_input.downcast::<PyBytes>() {
        Ok(b.as_bytes().to_vec())
    } else {
        Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(
            "xml_input must be str or bytes",
        ))
    }
}

fn validate_element_name(name: &str) -> PyResult<()> {
    if name.is_empty() || name.chars().any(|x| matches!(x, '<' | '>')) {
        return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
            "XML parse error: not well-formed (invalid element name)",
        ));
    }
    Ok(())
}

fn parse_xml_with_parser(
    py: Python,
    xml_bytes: &[u8],
    config: &ParseConfig,
    force_list: Option<PyObject>,
    strip_whitespace: bool,
    process_comments: bool,
) -> PyResult<PyObject> {
    let mut parser = XmlParser::new(config.clone(), force_list);
    let mut reader = Reader::from_reader(xml_bytes);
    reader
        .trim_text(strip_whitespace)
        .check_end_names(true)
        .check_comments(true)
        .expand_empty_elements(true);

    let mut buf = Vec::with_capacity(128);

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = std::str::from_utf8(e.name().into_inner())?;
                validate_element_name(name)?;
                let attrs: Vec<_> = e.attributes().collect::<Result<Vec<_>, _>>().map_err(|e| {
                    PyErr::new::<pyo3::exceptions::PyValueError, _>(format!("XML parse error: {e}"))
                })?;
                parser.start_element(py, name, &attrs)?;
            }
            Ok(Event::End(ref e)) => {
                let name = std::str::from_utf8(e.name().into_inner())?;
                validate_element_name(name)?;
                parser.end_element(py, name)?;
            }
            Ok(Event::Empty(ref e)) => {
                let name = std::str::from_utf8(e.name().into_inner())?;
                validate_element_name(name)?;

                let attrs: Vec<_> = e.attributes().collect::<Result<Vec<_>, _>>().map_err(|e| {
                    PyErr::new::<pyo3::exceptions::PyValueError, _>(format!("XML parse error: {e}"))
                })?;
                parser.start_element(py, name, &attrs)?;
                parser.end_element(py, name)?;
            }
            Ok(Event::Text(ref e)) => {
                let text = e.unescape().map_err(|e| {
                    PyErr::new::<pyo3::exceptions::PyValueError, _>(format!("XML parse error: {e}"))
                })?;
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
            Err(e) => {
                return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                    "XML parse error: {e}"
                )));
            }
            _ => {}
        }
        buf.clear();
    }

    match parser.stack.as_slice() {
        [one] => Ok(one.clone_ref(py)),
        [] => Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
            "XML parse error: no element found",
        )),
        [_, ..] => Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
            "XML parse error: unclosed element(s) found",
        )),
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
    item_depth = 0,
    comment_key = "#comment"
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
    force_list: Option<PyObject>,
    item_depth: usize,
    comment_key: &str,
) -> PyResult<PyObject> {
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
    };

    let xml_bytes = extract_xml_bytes(xml_input)?;

    let result = parse_xml_with_parser(
        py,
        &xml_bytes,
        &config,
        force_list,
        strip_whitespace,
        process_comments,
    )?;
    Ok(result)
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
}

impl XmlWriter {
    fn new(config: UnparseConfig) -> Self {
        Self {
            config,
            indent_level: 0,
            output: String::new(),
        }
    }

    fn write_header(&mut self) {
        if self.config.full_document {
            write!(
                &mut self.output,
                r#"<?xml version="1.0" encoding="{}"?>"#,
                self.config.encoding
            )
            .unwrap();
            // Always add newline after XML declaration (not just for pretty printing)
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

    fn write_element(
        &mut self,
        py: Python,
        tag: &str,
        value: &Bound<'_, PyAny>,
        needs_newline: bool,
    ) -> PyResult<()> {
        if self.config.pretty && needs_newline {
            self.output.push_str(&self.config.newl);
            self.write_indent();
        }

        // Check if value is None (empty element)
        if value.is_none() {
            if self.config.short_empty_elements {
                write!(&mut self.output, "<{tag}/>").unwrap();
            } else {
                write!(&mut self.output, "<{tag}></{tag}>").unwrap();
            }
            return Ok(());
        }

        // Check if value is a dict (element with attributes/children)
        if let Ok(dict) = value.downcast::<PyDict>() {
            self.write_dict_element(py, tag, dict)?;
        } else if let Ok(list) = value.downcast::<PyList>() {
            // Handle lists - create multiple elements with same tag
            for (i, item) in list.iter().enumerate() {
                self.write_element(py, tag, &item, i > 0 || needs_newline)?;
            }
        } else {
            if let Ok(bool_val) = value.extract::<bool>() {
                match bool_val {
                    true => write!(&mut self.output, "<{tag}>true</{tag}>").unwrap(),
                    false => write!(&mut self.output, "<{tag}>false</{tag}>").unwrap(),
                }
            } else {
                let val = value.str()?.to_string();
                write!(&mut self.output, "<{tag}>{}</{tag}>", escape_xml(&val)).unwrap()
            };
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

        // Separate attributes, text content, and child elements
        for (key, value) in dict {
            let key_str = key.str()?.to_string();

            if key_str.starts_with(&self.config.attr_prefix) {
                // Attribute - handle special Python types
                let attr_name = &key_str[self.config.attr_prefix.len()..];
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
                // Text content - handle special Python types
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
                // Child element
                child_elements.push((key_str, value));
            }
        }

        // Write opening tag with attributes
        self.output.push('<');
        self.output.push_str(tag);
        for (attr_name, attr_value) in attributes {
            write!(
                &mut self.output,
                r#" {attr_name}="{}""#,
                escape_xml_attr(&attr_value)
            )
            .unwrap();
        }

        if child_elements.is_empty() && text_content.is_none() {
            // Empty element
            if self.config.short_empty_elements {
                self.output.push_str("/>");
            } else {
                self.output.push_str("></");
                self.output.push_str(tag);
                self.output.push('>');
            }
        } else {
            self.output.push('>');

            // Write text content if present
            if let Some(text) = text_content {
                self.output.push_str(&escape_xml(&text));
            }

            // Write child elements
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

            // Write closing tag
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

fn escape_xml(text: &str) -> Cow<str> {
    let mut result: Option<String> = None;
    let mut last_pos = 0;

    for (i, ch) in text.char_indices() {
        match ch {
            '&' | '<' | '>' => {
                if result.is_none() {
                    let mut s = String::with_capacity(text.len() + 16);
                    s.push_str(&text[..i]);
                    result = Some(s);
                }
                let s = result.as_mut().unwrap();
                match ch {
                    '&' => s.push_str("&amp;"),
                    '<' => s.push_str("&lt;"),
                    '>' => s.push_str("&gt;"),
                    _ => unreachable!(),
                }
                last_pos = i + ch.len_utf8();
            }
            _ => {
                if let Some(ref mut s) = result {
                    s.push(ch);
                }
            }
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

fn escape_xml_attr(text: &str) -> Cow<str> {
    let mut result: Option<String> = None;
    let mut last_pos = 0;

    for (i, ch) in text.char_indices() {
        match ch {
            '&' | '<' | '>' | '"' => {
                if result.is_none() {
                    let mut s = String::with_capacity(text.len() + 20);
                    s.push_str(&text[..i]);
                    result = Some(s);
                }
                let s = result.as_mut().unwrap();
                match ch {
                    '&' => s.push_str("&amp;"),
                    '<' => s.push_str("&lt;"),
                    '>' => s.push_str("&gt;"),
                    '"' => s.push_str("&quot;"),
                    _ => unreachable!(),
                }
                last_pos = i + ch.len_utf8();
            }
            _ => {
                if let Some(ref mut s) = result {
                    s.push(ch);
                }
            }
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
    indent = "\t"
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
) -> PyResult<PyObject> {
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

    let mut writer = XmlWriter::new(config);

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
