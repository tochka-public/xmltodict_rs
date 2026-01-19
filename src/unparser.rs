use crate::config::UnparseConfig;
use crate::escape::{escape_xml, escape_xml_attr};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple};

pub struct XmlWriter {
    config: UnparseConfig,
    indent_level: usize,
    output: String,
    preprocessor: Option<Py<PyAny>>,
}

impl XmlWriter {
    pub fn new(config: UnparseConfig, preprocessor: Option<Py<PyAny>>) -> Self {
        Self {
            config,
            indent_level: 0,
            output: String::new(),
            preprocessor,
        }
    }

    pub fn write_header(&mut self) {
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

    pub fn write_element(
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

            if let Some(attr_name) = key_str.strip_prefix(self.config.attr_prefix.as_ref()) {
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

    pub fn finish(self) -> String {
        self.output
    }
}
