"""Type stubs for xmltodict_rs - High-performance XML to dict conversion library.

This module provides a Rust-based implementation of xmltodict functionality
with full type annotations for better IDE support and type checking.
"""

from collections.abc import Collection, Generator
from typing import Any, Callable, Protocol

class SupportsRead(Protocol):
    def read(self, size: int = ...) -> bytes: ...

XMLInput = str | bytes | SupportsRead | Generator[str | bytes, None, None]
XMLDict = dict[str, Any]
PostprocessorFunc = Callable[[list[str], str, Any], tuple[str, Any] | None]
PreprocessorFunc = Callable[[str, Any], tuple[str, Any] | None]

def parse(
    xml_input: XMLInput,
    encoding: str | None = None,
    process_namespaces: bool = False,
    namespace_separator: str = ":",
    disable_entities: bool = True,
    process_comments: bool = False,
    xml_attribs: bool = True,
    attr_prefix: str = "@",
    cdata_key: str = "#text",
    force_cdata: bool = False,
    cdata_separator: str = "",
    strip_whitespace: bool = True,
    force_list: bool | Collection[str] | Callable[[list[str], str, Any], bool] | None = None,
    postprocessor: PostprocessorFunc | None = None,
    item_depth: int = 0,
    comment_key: str = "#comment",
    namespaces: dict[str, str] | None = None,
) -> XMLDict:
    """Parse XML string or bytes into a Python dictionary.

    Args:
        xml_input: XML data as string or bytes to parse
        encoding: Character encoding (for compatibility, not used in Rust implementation)
        process_namespaces: If True, namespace prefixes are processed and expanded
        namespace_separator: Separator character between namespace and tag name (default ':')
        disable_entities: If True, XML entities are disabled for security (default True)
        process_comments: If True, XML comments are included in output with comment_key
        xml_attribs: If True, XML attributes are included in output (default True)
        attr_prefix: Prefix for attribute keys in output dict (default '@')
        cdata_key: Key name for text content in output dict (default '#text')
        force_cdata: If True, text content is always wrapped in dict with cdata_key
        cdata_separator: Separator for multiple text nodes (default '')
        strip_whitespace: If True, whitespace-only text is removed (default True)
        force_list: Control when to create lists for repeated elements:
            - None/False: automatic list creation for repeated elements
            - True: always create lists
            - set/list: create lists for specified tag names
            - Callable: custom function (path, key, value) -> bool
        postprocessor: Optional callback to transform parsed data:
            - Called with (path, key, value)
            - Should return (new_key, new_value) tuple or None to skip
        item_depth: Internal parameter for tracking parsing depth
        comment_key: Key name for XML comments in output (default '#comment')
        namespaces: Optional dict mapping namespace URIs to prefixes

    Returns:
        Dictionary representation of the XML structure

    Raises:
        ValueError: If XML is malformed or has parsing errors
        TypeError: If xml_input is not str or bytes

    Examples:
        >>> parse('<root><item>value</item></root>')
        {'root': {'item': 'value'}}

        >>> parse('<root id="1"><item>A</item><item>B</item></root>')
        {'root': {'@id': '1', 'item': ['A', 'B']}}

        >>> parse('<root>text</root>', force_cdata=True)
        {'root': {'#text': 'text'}}

        >>> def postproc(path, key, value):
        ...     return (key.upper(), value)
        >>> parse('<root><item>value</item></root>', postprocessor=postproc)
        {'ROOT': {'ITEM': 'value'}}
    """
    ...

def unparse(
    input_dict: XMLDict,
    output: str | None = None,
    encoding: str = "utf-8",
    full_document: bool = True,
    short_empty_elements: bool = False,
    attr_prefix: str = "@",
    cdata_key: str = "#text",
    pretty: bool = False,
    newl: str = "\n",
    indent: str = "\t",
    preprocessor: PreprocessorFunc | None = None,
) -> str:
    r"""Convert Python dictionary back to XML string.

    Args:
        input_dict: Dictionary to convert to XML (must have exactly one root key if full_document=True)
        output: Optional file-like object to write to (for compatibility, returns string anyway)
        encoding: Character encoding for XML declaration (default 'utf-8')
        full_document: If True, includes XML declaration (default True)
        short_empty_elements: If True, empty elements use <tag/> format (default False)
        attr_prefix: Prefix used to identify attribute keys (default '@')
        cdata_key: Key name that contains text content (default '#text')
        pretty: If True, output is formatted with indentation (default False)
        newl: Newline character for pretty printing (default '\n')
        indent: Indentation string for pretty printing (default '\t')
        preprocessor: Optional callback to transform data before unparsing:
            - Called with (key, value)
            - Should return (new_key, new_value) tuple or None to skip

    Returns:
        XML string representation of the dictionary

    Raises:
        ValueError: If full_document=True and dict doesn't have exactly one root element
        TypeError: If input_dict is not a dictionary

    Examples:
        >>> unparse({'root': {'item': 'value'}})
        '<?xml version="1.0" encoding="utf-8"?>\\n<root><item>value</item></root>'

        >>> unparse({'root': {'@id': '1', 'item': 'value'}})
        '<?xml version="1.0" encoding="utf-8"?>\\n<root id="1"><item>value</item></root>'

        >>> unparse({'root': None}, short_empty_elements=True)
        '<?xml version="1.0" encoding="utf-8"?>\\n<root/>'

        >>> unparse({'root': {'item': ['A', 'B']}})
        '<?xml version="1.0" encoding="utf-8"?>\\n<root><item>A</item><item>B</item></root>'

        >>> unparse({'root': {'child': 'value'}}, pretty=True)
        '<?xml version="1.0" encoding="utf-8"?>\\n<root>\\n\\t<child>value</child>\\n</root>'
    """
    ...

__all__ = ["parse", "unparse"]
