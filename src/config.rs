use std::collections::HashMap;
use std::fmt;
use std::ops::Deref;

/// Newtype for attribute prefix (e.g., "@" for "@id", "@class")
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttrPrefix(String);

impl AttrPrefix {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }
}

impl Default for AttrPrefix {
    fn default() -> Self {
        Self("@".to_owned())
    }
}

impl Deref for AttrPrefix {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<str> for AttrPrefix {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AttrPrefix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Newtype for CDATA key (e.g., "#text")
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CdataKey(String);

impl CdataKey {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }
}

impl Default for CdataKey {
    fn default() -> Self {
        Self("#text".to_owned())
    }
}

impl Deref for CdataKey {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<str> for CdataKey {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CdataKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl PartialEq<str> for CdataKey {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<CdataKey> for str {
    fn eq(&self, other: &CdataKey) -> bool {
        self == other.0
    }
}

impl PartialEq<String> for CdataKey {
    fn eq(&self, other: &String) -> bool {
        self.0 == *other
    }
}

impl PartialEq<CdataKey> for String {
    fn eq(&self, other: &CdataKey) -> bool {
        *self == other.0
    }
}

/// Newtype for comment key (e.g., "#comment")
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommentKey(String);

impl CommentKey {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }
}

impl Default for CommentKey {
    fn default() -> Self {
        Self("#comment".to_owned())
    }
}

impl Deref for CommentKey {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<str> for CommentKey {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CommentKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Newtype for namespace separator (e.g., ":")
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NamespaceSeparator(String);

impl NamespaceSeparator {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }
}

impl Default for NamespaceSeparator {
    fn default() -> Self {
        Self(":".to_owned())
    }
}

impl Deref for NamespaceSeparator {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<str> for NamespaceSeparator {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for NamespaceSeparator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Configuration for XML parsing.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone)]
pub struct ParseConfig {
    pub xml_attribs: bool,
    pub attr_prefix: AttrPrefix,
    pub cdata_key: CdataKey,
    pub force_cdata: bool,
    pub cdata_separator: String,
    pub strip_whitespace: bool,
    pub namespace_separator: NamespaceSeparator,
    pub process_namespaces: bool,
    pub comment_key: CommentKey,
    pub namespaces: Option<HashMap<String, String>>,
}

impl Default for ParseConfig {
    fn default() -> Self {
        Self {
            xml_attribs: true,
            attr_prefix: AttrPrefix::default(),
            cdata_key: CdataKey::default(),
            force_cdata: false,
            cdata_separator: String::new(),
            strip_whitespace: true,
            namespace_separator: NamespaceSeparator::default(),
            process_namespaces: false,
            comment_key: CommentKey::default(),
            namespaces: None,
        }
    }
}

pub struct UnparseConfig {
    pub encoding: String,
    pub full_document: bool,
    pub short_empty_elements: bool,
    pub attr_prefix: AttrPrefix,
    pub cdata_key: CdataKey,
    pub pretty: bool,
    pub newl: String,
    pub indent: String,
}
