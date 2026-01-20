import pytest
import xmltodict

import xmltodict_rs


def compare_roundtrip(obj: dict, parse_kwargs=None, unparse_kwargs=None) -> None:
    parse_kwargs = parse_kwargs or {}
    unparse_kwargs = unparse_kwargs or {}

    original_xml = xmltodict.unparse(obj, **unparse_kwargs)
    original_roundtrip = xmltodict.parse(original_xml, **parse_kwargs)

    rust_xml = xmltodict_rs.unparse(obj, **unparse_kwargs)
    rust_roundtrip = xmltodict_rs.parse(rust_xml, **parse_kwargs)

    assert rust_roundtrip == original_roundtrip == obj, (
        f"\nOriginal object: {obj!r}\n"
        f"Parse kwargs: {parse_kwargs}\n"
        f"Unparse kwargs: {unparse_kwargs}\n"
        f"Original XML: {original_xml!r}\n"
        f"Rust XML:     {rust_xml!r}\n"
        f"Original roundtrip: {original_roundtrip!r}\n"
        f"Rust roundtrip:     {rust_roundtrip!r}"
    )


SIMPLE_OBJECTS = [
    {"a": None},
    {"a": "b"},
    {"root": "hello"},
    {"item": "123"},
    {"text": "no_spaces"},
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


@pytest.mark.parametrize("obj", SIMPLE_OBJECTS)
def test_simple_roundtrip(obj):
    compare_roundtrip(obj)


@pytest.mark.parametrize("obj", ATTRIBUTE_OBJECTS)
def test_attribute_roundtrip(obj):
    compare_roundtrip(obj)


@pytest.mark.parametrize("obj", TEXT_CONTENT_OBJECTS)
def test_text_content_roundtrip(obj):
    compare_roundtrip(obj, parse_kwargs={"force_cdata": True})


def test_whitespace_roundtrip_with_strip_false():
    obj = {"text": "  spaces  "}

    original_xml = xmltodict.unparse(obj)
    original_roundtrip = xmltodict.parse(original_xml, strip_whitespace=False)

    rust_xml = xmltodict_rs.unparse(obj)
    rust_roundtrip = xmltodict_rs.parse(rust_xml, strip_whitespace=False)

    assert rust_roundtrip == original_roundtrip == obj


def test_whitespace_default_behavior():
    obj = {"text": "  spaces  "}

    original_xml = xmltodict.unparse(obj)
    original_parsed = xmltodict.parse(original_xml)

    rust_xml = xmltodict_rs.unparse(obj)
    rust_parsed = xmltodict_rs.parse(rust_xml)

    assert original_parsed == rust_parsed == {"text": "spaces"}


def test_nested_roundtrip():
    obj = {"root": {"child": {"grandchild": "value"}}}
    compare_roundtrip(obj)


def test_list_roundtrip():
    obj = {"root": {"item": ["1", "2", "3"]}}
    compare_roundtrip(obj)


def test_mixed_roundtrip():
    obj = {"root": {"@attr": "value", "child": "text"}}
    compare_roundtrip(obj)


def test_deeply_nested_roundtrip():
    obj = {"a": {"b": {"c": {"d": {"e": "deep"}}}}}
    compare_roundtrip(obj)


def test_unicode_roundtrip():
    obj = {"root": "Hello 世界"}
    compare_roundtrip(obj)


def test_cyrillic_roundtrip():
    obj = {"root": "Привет мир"}
    compare_roundtrip(obj)


def test_multiple_children_roundtrip():
    obj = {"root": {"a": "1", "b": "2", "c": "3"}}
    compare_roundtrip(obj)


def test_attributes_and_text_roundtrip():
    obj = {"item": {"@id": "123", "@name": "test", "#text": "content"}}
    compare_roundtrip(obj, parse_kwargs={"force_cdata": True})


def test_list_with_attributes_roundtrip():
    obj = {"root": {"item": [{"@id": "1"}, {"@id": "2"}, {"@id": "3"}]}}
    compare_roundtrip(obj)


def test_empty_element_roundtrip():
    obj = {"root": None}
    compare_roundtrip(obj)


def test_empty_nested_element_roundtrip():
    obj = {"root": {"empty": None}}
    compare_roundtrip(obj)


def test_complex_structure_roundtrip():
    obj = {
        "catalog": {
            "@version": "2.0",
            "product": [
                {"@id": "1", "name": "Item 1", "price": "10.00"},
                {"@id": "2", "name": "Item 2", "price": "20.00"},
            ],
        }
    }
    compare_roundtrip(obj)


def test_special_chars_in_text_roundtrip():
    obj = {"root": 'text with <special> & "chars"'}
    original_xml = xmltodict.unparse(obj)
    original_parsed = xmltodict.parse(original_xml)

    rust_xml = xmltodict_rs.unparse(obj)
    rust_parsed = xmltodict_rs.parse(rust_xml)

    assert original_parsed == rust_parsed


def test_special_chars_in_attr_roundtrip_simple():
    obj = {"root": {"@attr": "simple value"}}
    original_xml = xmltodict.unparse(obj)
    original_parsed = xmltodict.parse(original_xml)

    rust_xml = xmltodict_rs.unparse(obj)
    rust_parsed = xmltodict_rs.parse(rust_xml)

    assert original_parsed == rust_parsed


def test_special_chars_in_attr_roundtrip():
    obj = {"root": {"@attr": "value with <angle> & ampersand"}}
    original_xml = xmltodict.unparse(obj)
    original_parsed = xmltodict.parse(original_xml)

    rust_xml = xmltodict_rs.unparse(obj)
    rust_parsed = xmltodict_rs.parse(rust_xml)

    assert original_parsed == rust_parsed
