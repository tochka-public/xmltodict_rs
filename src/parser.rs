use crate::config::ParseConfig;
use crate::error::expat_error;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple};
use pyo3::IntoPyObjectExt;
use quick_xml::name::PrefixDeclaration;
use std::collections::HashMap;

/// Represents an XML namespace prefix.
/// Default namespace has empty string as key in the namespace map.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum NamespacePrefix {
    /// Default namespace (no prefix, represented as empty string)
    Default,
    /// Named namespace with explicit prefix
    Named(String),
}

impl NamespacePrefix {
    /// Returns the string representation of the prefix.
    /// Default namespace returns empty string, named returns the prefix.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Default => "",
            Self::Named(s) => s,
        }
    }
}

impl From<&str> for NamespacePrefix {
    fn from(s: &str) -> Self {
        if s.is_empty() {
            Self::Default
        } else {
            Self::Named(s.to_string())
        }
    }
}

impl From<String> for NamespacePrefix {
    fn from(s: String) -> Self {
        if s.is_empty() {
            Self::Default
        } else {
            Self::Named(s)
        }
    }
}

pub struct XmlParser {
    config: ParseConfig,
    force_list: Option<Py<PyAny>>,
    postprocessor: Option<Py<PyAny>>,
    pub stack: Vec<Py<PyAny>>,
    pub path: Vec<String>,
    pub text_stack: Vec<Vec<String>>,
    pub namespace_stack: Vec<HashMap<String, String>>,
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
            .unwrap_or((NamespacePrefix::Default.as_str(), full_name));
        if let Some(uri) = ns_map.get(prefix) {
            let mapped = self
                .config
                .namespaces
                .as_ref()
                .and_then(|m| m.get(uri))
                .unwrap_or(uri);
            if mapped.is_empty() {
                return name.to_string();
            }
            return format!("{mapped}{ns_sep}{name}");
        }
        full_name.to_string()
    }

    pub fn start_element(
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
                let value_string = attr
                    .unescape_value()
                    .map_err(|e| expat_error(py, e.to_string()))?
                    .to_string();

                if self.config.process_namespaces {
                    if let Some(ns) = key.as_namespace_binding() {
                        match ns {
                            PrefixDeclaration::Default => {
                                current_ns_map.insert(
                                    NamespacePrefix::Default.as_str().to_string(),
                                    value_string,
                                );
                            }
                            PrefixDeclaration::Named(name) => {
                                let key_string = String::from_utf8(name.to_vec())?;
                                if !set_xmlns_item {
                                    if let Some(m) = self.config.namespaces.as_ref() {
                                        set_xmlns_item = !m.contains_key(&value_string);
                                    }
                                }
                                current_ns_map.insert(key_string, value_string);
                            }
                        }
                        continue;
                    }
                }

                let key_str = String::from_utf8(key.into_inner().to_vec())?;
                if self.config.process_namespaces && !set_xmlns_item && key_str.contains(':') {
                    if let Some((prefix, _)) = key_str.split_once(':') {
                        if let Some(uri) = current_ns_map.get(prefix) {
                            if let Some(m) = self.config.namespaces.as_ref() {
                                if !m.contains_key(uri) {
                                    set_xmlns_item = true;
                                }
                            } else {
                                set_xmlns_item = true;
                            }
                        }
                    }
                }
                normal_attrs.push((key_str, value_string));
            }
        }

        if self.config.xml_attribs && set_xmlns_item {
            let ns_py = PyDict::new(py);
            for (key, value) in &current_ns_map {
                ns_py.set_item(key, value)?;
            }
            let xmlns_key = format!("{}xmlns", self.config.attr_prefix);
            element_dict.set_item(xmlns_key, ns_py)?;
        }

        self.namespace_stack.push(current_ns_map);

        if self.config.xml_attribs {
            for (key, value) in normal_attrs {
                let attr_local_name = if self.config.process_namespaces
                    && key.contains(self.config.namespace_separator.as_ref())
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

    pub fn end_element(&mut self, py: Python, name: &str) -> PyResult<()> {
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

    pub fn characters(&mut self, data: &str) {
        if let Some(current_text) = self.text_stack.last_mut() {
            current_text.push(data.to_string());
        }
    }

    pub fn comment(&self, py: Python, comment: &str) -> PyResult<()> {
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
