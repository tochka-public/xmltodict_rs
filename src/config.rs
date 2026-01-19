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
        Self("@".to_string())
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
        Self("#text".to_string())
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
        Self("#comment".to_string())
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
        Self(":".to_string())
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
/// Some fields are kept for API compatibility with xmltodict but not used in current implementation.
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
    #[allow(dead_code)]
    pub process_comments: bool,
    pub comment_key: CommentKey,
    #[allow(dead_code)]
    pub item_depth: usize,
    #[allow(dead_code)]
    pub disable_entities: bool,
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
            process_comments: false,
            comment_key: CommentKey::default(),
            item_depth: 0,
            disable_entities: true,
            namespaces: None,
        }
    }
}

#[allow(dead_code)]
impl ParseConfig {
    /// Create a new builder for `ParseConfig` with default values.
    #[must_use]
    pub fn builder() -> ParseConfigBuilder {
        ParseConfigBuilder::default()
    }
}

/// Builder for `ParseConfig` with fluent API.
#[allow(dead_code)]
#[derive(Default)]
pub struct ParseConfigBuilder {
    config: ParseConfig,
}

#[allow(dead_code)]
impl ParseConfigBuilder {
    /// Set whether to include XML attributes in the output.
    #[must_use]
    pub fn xml_attribs(mut self, value: bool) -> Self {
        self.config.xml_attribs = value;
        self
    }

    /// Set the prefix for attribute keys (default: "@").
    #[must_use]
    pub fn attr_prefix(mut self, value: impl Into<String>) -> Self {
        self.config.attr_prefix = AttrPrefix::new(value);
        self
    }

    /// Set the key for text content (default: "#text").
    #[must_use]
    pub fn cdata_key(mut self, value: impl Into<String>) -> Self {
        self.config.cdata_key = CdataKey::new(value);
        self
    }

    /// Set whether to always wrap text content in a dict with `cdata_key`.
    #[must_use]
    pub fn force_cdata(mut self, value: bool) -> Self {
        self.config.force_cdata = value;
        self
    }

    /// Set the separator for joining multiple text nodes.
    #[must_use]
    pub fn cdata_separator(mut self, value: impl Into<String>) -> Self {
        self.config.cdata_separator = value.into();
        self
    }

    /// Set whether to strip whitespace from text content.
    #[must_use]
    pub fn strip_whitespace(mut self, value: bool) -> Self {
        self.config.strip_whitespace = value;
        self
    }

    /// Set the separator between namespace and local name (default: ":").
    #[must_use]
    pub fn namespace_separator(mut self, value: impl Into<String>) -> Self {
        self.config.namespace_separator = NamespaceSeparator::new(value);
        self
    }

    /// Set whether to process XML namespaces.
    #[must_use]
    pub fn process_namespaces(mut self, value: bool) -> Self {
        self.config.process_namespaces = value;
        self
    }

    /// Set whether to process XML comments.
    #[must_use]
    pub fn process_comments(mut self, value: bool) -> Self {
        self.config.process_comments = value;
        self
    }

    /// Set the key for comment content (default: "#comment").
    #[must_use]
    pub fn comment_key(mut self, value: impl Into<String>) -> Self {
        self.config.comment_key = CommentKey::new(value);
        self
    }

    /// Set the item depth for streaming parsing.
    #[must_use]
    pub fn item_depth(mut self, value: usize) -> Self {
        self.config.item_depth = value;
        self
    }

    /// Set whether to disable entity expansion.
    #[must_use]
    pub fn disable_entities(mut self, value: bool) -> Self {
        self.config.disable_entities = value;
        self
    }

    /// Set namespace URI to prefix mappings.
    #[must_use]
    pub fn namespaces(mut self, value: Option<HashMap<String, String>>) -> Self {
        self.config.namespaces = value;
        self
    }

    /// Build the final `ParseConfig`.
    #[must_use]
    pub fn build(self) -> ParseConfig {
        self.config
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
