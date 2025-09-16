"""
Compatibility tests comparing xmltodict_rs.unparse with original xmltodict.unparse
This ensures our Rust implementation produces identical results for unparse
"""

import re
from collections import OrderedDict

import pytest
import xmltodict
import xmltodict_rs

_HEADER_RE = re.compile(r"^[^\n]*\n")


def _strip(fullxml):
    """Remove XML header line for comparison"""
    return _HEADER_RE.sub("", fullxml)


def compare_unparse(obj: dict, **kwargs) -> None:
    """Compare xmltodict_rs.unparse with xmltodict.unparse"""
    try:
        original = xmltodict.unparse(obj, **kwargs)
        rust_impl = xmltodict_rs.unparse(obj, **kwargs)

        # Strip headers for comparison if not testing full document
        if not kwargs.get("full_document", True):
            original_clean = original
            rust_clean = rust_impl
        else:
            original_clean = _strip(original)
            rust_clean = _strip(rust_impl)

        assert rust_clean == original_clean, (
            f"\nObject: {obj!r}\n"
            f"Kwargs: {kwargs}\n"
            f"Original: {original!r}\n"
            f"Rust:     {rust_impl!r}\n"
            f"Original clean: {original_clean!r}\n"
            f"Rust clean:     {rust_clean!r}"
        )
    except Exception as e:
        # If original throws exception, rust should too
        with pytest.raises(type(e)):
            xmltodict_rs.unparse(obj, **kwargs)


def compare_roundtrip(obj: dict, parse_kwargs=None, unparse_kwargs=None) -> None:
    """Test that parse(unparse(obj)) == obj for both implementations"""
    parse_kwargs = parse_kwargs or {}
    unparse_kwargs = unparse_kwargs or {}

    # Test original implementation roundtrip
    original_xml = xmltodict.unparse(obj, **unparse_kwargs)
    original_roundtrip = xmltodict.parse(original_xml, **parse_kwargs)

    # Test rust implementation roundtrip
    rust_xml = xmltodict_rs.unparse(obj, **unparse_kwargs)
    rust_roundtrip = xmltodict_rs.parse(rust_xml, **parse_kwargs)

    # Both should produce the same result
    assert rust_roundtrip == original_roundtrip == obj, (
        f"\nOriginal object: {obj!r}\n"
        f"Parse kwargs: {parse_kwargs}\n"
        f"Unparse kwargs: {unparse_kwargs}\n"
        f"Original XML: {original_xml!r}\n"
        f"Rust XML:     {rust_xml!r}\n"
        f"Original roundtrip: {original_roundtrip!r}\n"
        f"Rust roundtrip:     {rust_roundtrip!r}"
    )


# Test data for parametrized tests
SIMPLE_OBJECTS = [
    {"a": None},
    {"a": "b"},
    {"root": "hello"},
    {"item": "123"},
    {"text": "no_spaces"},  # Changed from '  spaces  ' to avoid strip_whitespace issue
]

ATTRIBUTE_OBJECTS = [
    {"a": {"@href": "x"}},
    {"root": {"@id": "123"}},
    {"item": {"@name": "test", "@value": "data"}},
    {"element": {"@attr1": "val1", "@attr2": "val2", "#text": "content"}},
]

TEXT_CONTENT_OBJECTS = [
    {"a": {"#text": "y"}},
    {"root": {"@href": "x", "#text": "y"}},
]

LIST_OBJECTS = [
    {"a": {"b": ["1", "2", "3"]}},
    {"root": {"item": [{"@id": "1"}, {"@id": "2"}]}},
]

NESTED_OBJECTS = [
    {"a": {"b": "1", "c": "2"}},
    {"a": {"b": {"c": {"@a": "x", "#text": "y"}}}},
    {"root": {"child1": {"subchild": "value1"}, "child2": "value2"}},
]

SPECIAL_VALUE_OBJECTS = [
    {"a": 1},
    {"a": True},
    {"a": False},
    {"root": {"@attr": 1}},
]


# Parametrized tests
@pytest.mark.parametrize("obj", SIMPLE_OBJECTS)
def test_simple_objects(obj):
    """Test simple objects with no attributes or children"""
    compare_unparse(obj)


@pytest.mark.parametrize("obj", ATTRIBUTE_OBJECTS)
def test_attribute_objects(obj):
    """Test objects with attributes"""
    compare_unparse(obj)


@pytest.mark.parametrize("obj", TEXT_CONTENT_OBJECTS)
def test_text_content_objects(obj):
    """Test objects with text content"""
    compare_unparse(obj)


@pytest.mark.parametrize("obj", LIST_OBJECTS)
def test_list_objects(obj):
    """Test objects with lists (repeated elements)"""
    compare_unparse(obj)


@pytest.mark.parametrize("obj", NESTED_OBJECTS)
def test_nested_objects(obj):
    """Test deeply nested objects"""
    compare_unparse(obj)


@pytest.mark.parametrize("obj", SPECIAL_VALUE_OBJECTS)
def test_special_value_objects(obj):
    """Test objects with non-string values"""
    compare_unparse(obj)


# Configuration parameter tests
@pytest.mark.parametrize("obj", SIMPLE_OBJECTS[:3])
def test_short_empty_elements(obj):
    """Test short_empty_elements parameter"""
    if obj[next(iter(obj.keys()))] is None:  # Only test with empty elements
        compare_unparse(obj, short_empty_elements=True)


@pytest.mark.parametrize("obj", SIMPLE_OBJECTS[:3])
def test_full_document_false(obj):
    """Test full_document=False parameter"""
    compare_unparse(obj, full_document=False)


@pytest.mark.parametrize("obj", SIMPLE_OBJECTS[:2])
def test_encoding(obj):
    """Test different encoding parameter"""
    compare_unparse(obj, encoding="utf-8")


# Roundtrip tests
@pytest.mark.parametrize("obj", SIMPLE_OBJECTS)
def test_simple_roundtrip(obj):
    """Test roundtrip: obj -> XML -> obj for simple objects"""
    compare_roundtrip(obj)


@pytest.mark.parametrize("obj", ATTRIBUTE_OBJECTS)
def test_attribute_roundtrip(obj):
    """Test roundtrip for objects with attributes"""
    compare_roundtrip(obj)


@pytest.mark.parametrize("obj", TEXT_CONTENT_OBJECTS)
def test_text_content_roundtrip(obj):
    """Test roundtrip for objects with text content"""
    # Use force_cdata for proper roundtrip with #text
    compare_roundtrip(obj, parse_kwargs={"force_cdata": True})


# Error cases
def test_empty_dict():
    """Test empty dictionary should raise error with full_document=True"""
    with pytest.raises(ValueError):
        xmltodict.unparse({})
    with pytest.raises(ValueError):
        xmltodict_rs.unparse({})


def test_multiple_roots():
    """Test multiple root elements should raise error with full_document=True"""
    obj = {"a": "1", "b": "2"}
    with pytest.raises(ValueError):
        xmltodict.unparse(obj)
    with pytest.raises(ValueError):
        xmltodict_rs.unparse(obj)


def test_empty_dict_no_full_document():
    """Test empty dict with full_document=False"""
    compare_unparse({}, full_document=False)


def test_multiple_roots_no_full_document():
    """Test multiple roots with full_document=False"""
    obj = OrderedDict([("a", 1), ("b", 2)])
    compare_unparse(obj, full_document=False)


# Special cases from original tests
def test_non_string_values():
    """Test non-string values are converted properly"""
    compare_unparse({"a": 1})
    compare_unparse({"a": {"@attr": 1}})


def test_boolean_values():
    """Test boolean values are converted properly"""
    compare_unparse({"x": True})
    compare_unparse({"x": False})


def test_pretty_printing():
    """Test pretty printing if supported"""
    obj = {"a": {"b": [{"c": [1, 2]}, 3], "x": "y"}}

    def clean(x: str) -> str:
        return "\n".join(x for x in x.splitlines() if x.strip())

    original = clean(xmltodict.unparse(obj, pretty=True))
    rust_impl = clean(xmltodict_rs.unparse(obj, pretty=True))
    assert original == rust_impl
