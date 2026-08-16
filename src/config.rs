use std::collections::HashMap;

macro_rules! config_newtype {
    ($(#[$doc:meta])* $name:ident, $default:literal) => {
        $(#[$doc])*
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct $name(String);

        impl $name {
            pub fn new(s: impl Into<String>) -> Self {
                Self(s.into())
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self($default.to_owned())
            }
        }

        impl std::ops::Deref for $name {
            type Target = str;
            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}

config_newtype!(
    /// Attribute prefix (e.g., "@" for "@id", "@class")
    AttrPrefix, "@"
);
config_newtype!(
    /// Key for text content (e.g., "#text")
    CdataKey, "#text"
);
config_newtype!(
    /// Key for comment content (e.g., "#comment")
    CommentKey, "#comment"
);
config_newtype!(
    /// Separator between namespace and local name (e.g., ":")
    NamespaceSeparator, ":"
);

impl PartialEq<CdataKey> for String {
    fn eq(&self, other: &CdataKey) -> bool {
        *self == other.0
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
