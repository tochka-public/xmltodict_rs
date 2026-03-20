use crate::encoding::error::EncodingError;
use encoding_rs::Encoding;
use memchr::memmem::Finder;
use pyo3::prelude::*;
use std::borrow::Cow;
use std::sync::LazyLock;

pub static XML_DECL_END: LazyLock<Finder<'static>> = LazyLock::new(|| Finder::new(b"?>"));
static XML_DECL_ENCODING: LazyLock<Finder<'static>> = LazyLock::new(|| Finder::new(b"encoding"));

pub fn identify_encoding_from_opening_bytes(data: &[u8]) -> Option<&'static Encoding> {
    use encoding_rs::{UTF_16BE, UTF_16LE};

    if let Some((enc, _)) = Encoding::for_bom(data) {
        Some(enc)
    } else if data.starts_with(b"\x3C\x00") {
        // '<' in UTF-16 LE without BOM
        Some(UTF_16LE)
    } else if data.starts_with(b"\x00\x3C") {
        // '<' in UTF-16 BE without BOM
        Some(UTF_16BE)
    } else {
        parse_xml_encoding_declaration(data)
    }
}

/// Returns the encoding declared in the XML declaration.
///
/// ```
/// XMLDecl ::= '<?xml' VersionInfo EncodingDecl? SDDecl? S? '?>'
/// EncodingDecl ::= S 'encoding' Eq ('"' EncName '"' | "'" EncName "'")
/// ```
fn parse_xml_encoding_declaration(data: &[u8]) -> Option<&'static Encoding> {
    let prefix = b"<?xml";
    if data.len() < prefix.len() || !data.get(..prefix.len())?.eq_ignore_ascii_case(prefix) {
        return None;
    }

    let decl_end = XML_DECL_END.find(data)?;
    let decl = data.get(..decl_end)?;
    let enc_pos = XML_DECL_ENCODING.find(decl)?;
    let after_enc = decl.get(enc_pos + 8..)?;

    let after_eq = skip_whitespace_and_eq(after_enc)?;
    let (&quote, rest) = after_eq.split_first()?;
    if quote != b'"' && quote != b'\'' {
        return None;
    }
    let end = memchr::memchr(quote, rest)?;

    Encoding::for_label(rest.get(..end)?)
}

fn skip_whitespace_and_eq(data: &[u8]) -> Option<&[u8]> {
    let rest = data.trim_ascii_start();
    let rest = rest.strip_prefix(b"=")?;
    Some(rest.trim_ascii_start())
}

enum PythonEncodingAlias {
    Utf8,
    Utf16Le,
    Utf16Be,
    WindowsCodepage(String),
    Other(String),
}

impl PythonEncodingAlias {
    fn from_label(label: &str) -> Self {
        let lower = label.to_ascii_lowercase();
        let stripped: String = lower.chars().filter(|c| *c != '-' && *c != '_').collect();
        match stripped.as_str() {
            "utf8" => Self::Utf8,
            "utf16le" | "utf16" => Self::Utf16Le,
            "utf16be" => Self::Utf16Be,
            s if s.len() > 2 && s.starts_with("cp") => {
                Self::WindowsCodepage(s.get(2..).unwrap_or_default().to_string())
            }
            _ => Self::Other(lower),
        }
    }

    fn as_whatwg_label(&self) -> &str {
        match self {
            Self::Utf8 => "utf-8",
            Self::Utf16Le => "utf-16le",
            Self::Utf16Be => "utf-16be",
            Self::WindowsCodepage(num) => {
                unreachable!("WindowsCodepage should be handled by lookup_encoding, got cp{num}")
            }
            Self::Other(label) => label,
        }
    }
}

pub fn lookup_encoding(label: &str) -> PyResult<&'static Encoding> {
    let alias = PythonEncodingAlias::from_label(label);

    let result = match &alias {
        PythonEncodingAlias::WindowsCodepage(num) => {
            let whatwg = format!("windows-{num}");
            Encoding::for_label(whatwg.as_bytes())
        }
        PythonEncodingAlias::Utf8
        | PythonEncodingAlias::Utf16Le
        | PythonEncodingAlias::Utf16Be
        | PythonEncodingAlias::Other(_) => Encoding::for_label(alias.as_whatwg_label().as_bytes()),
    };

    result
        .or_else(|| Encoding::for_label(label.as_bytes()))
        .ok_or_else(|| {
            PyErr::new::<pyo3::exceptions::PyLookupError, _>(format!("unknown encoding: {label}"))
        })
}

pub fn decode_bytes_to_utf8<'a>(
    data: &'a [u8],
    encoding: Option<&'static Encoding>,
) -> Result<Cow<'a, str>, EncodingError> {
    let enc = encoding
        .or_else(|| identify_encoding_from_opening_bytes(data))
        .unwrap_or(encoding_rs::UTF_8);
    let (result, _used_encoding, had_errors) = enc.decode(data);
    if had_errors {
        Err(EncodingError(enc))
    } else {
        Ok(result)
    }
}

pub mod error {
    use super::{Encoding, PyErr};

    #[derive(Debug, Clone, Copy)]
    pub struct EncodingError(pub &'static Encoding);

    impl std::fmt::Display for EncodingError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let enc = self.0;
            if enc == encoding_rs::UTF_8 {
                f.write_str("invalid UTF-8 data in XML input")
            } else {
                write!(
                    f,
                    "failed to decode XML input from encoding: {}",
                    enc.name()
                )
            }
        }
    }

    impl std::error::Error for EncodingError {}

    impl From<EncodingError> for PyErr {
        fn from(err: EncodingError) -> Self {
            PyErr::new::<pyo3::exceptions::PyValueError, _>(err.to_string())
        }
    }
}
