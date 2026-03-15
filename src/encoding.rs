use encoding_rs::Encoding;
use pyo3::prelude::*;
use std::borrow::Cow;

enum XmlBytePrefix {
    /// UTF-8 BOM: EF BB BF
    Utf8,
    /// UTF-16 LE BOM: FF FE
    Utf16Le,
    /// UTF-16 BE BOM: FE FF
    Utf16Be,
    /// '<' in UTF-16 LE without BOM: 3C 00
    Utf16LeAngleBracket,
    /// '<' in UTF-16 BE without BOM: 00 3C
    Utf16BeAngleBracket,
}

impl XmlBytePrefix {
    const ALL: &[Self] = &[
        Self::Utf8,
        Self::Utf16Le,
        Self::Utf16Be,
        Self::Utf16LeAngleBracket,
        Self::Utf16BeAngleBracket,
    ];

    fn bytes(&self) -> &'static [u8] {
        match self {
            Self::Utf8 => b"\xEF\xBB\xBF",
            Self::Utf16Le => b"\xFF\xFE",
            Self::Utf16Be => b"\xFE\xFF",
            Self::Utf16LeAngleBracket => b"\x3C\x00",
            Self::Utf16BeAngleBracket => b"\x00\x3C",
        }
    }

    fn encoding(&self) -> &'static Encoding {
        match self {
            Self::Utf8 => encoding_rs::UTF_8,
            Self::Utf16Le | Self::Utf16LeAngleBracket => encoding_rs::UTF_16LE,
            Self::Utf16Be | Self::Utf16BeAngleBracket => encoding_rs::UTF_16BE,
        }
    }
}

fn detect_encoding_from_bytes(data: &[u8]) -> Option<&'static Encoding> {
    for sig in XmlBytePrefix::ALL {
        if data.starts_with(sig.bytes()) {
            return Some(sig.encoding());
        }
    }
    parse_encoding_from_declaration(data)
}

fn parse_encoding_from_declaration(data: &[u8]) -> Option<&'static Encoding> {
    let prefix = b"<?xml";
    if data.len() < prefix.len() || !data.get(..prefix.len())?.eq_ignore_ascii_case(prefix) {
        return None;
    }

    let decl_end = memchr::memmem::find(data, b"?>")?;
    let decl = data.get(..decl_end)?;
    let enc_pos = memchr::memmem::find(decl, b"encoding")?;
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

pub fn decode_bytes_to_utf8<'a>(data: &'a [u8], encoding: Option<&str>) -> PyResult<Cow<'a, str>> {
    let enc = match encoding {
        Some(label) => lookup_encoding(label)?,
        None => detect_encoding_from_bytes(data).unwrap_or(encoding_rs::UTF_8),
    };

    let (result, _used_encoding, had_errors) = enc.decode(data);
    if had_errors {
        let msg = if enc == encoding_rs::UTF_8 {
            "invalid UTF-8 data in XML input".to_string()
        } else {
            format!("failed to decode XML input from encoding: {}", enc.name())
        };
        return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(msg));
    }
    Ok(result)
}
