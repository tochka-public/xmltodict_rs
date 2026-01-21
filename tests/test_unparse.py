import re
from collections import OrderedDict

import pytest
import xmltodict

import xmltodict_rs

_HEADER_RE = re.compile(r"^[^\n]*\n")


def _strip(fullxml):
    return _HEADER_RE.sub("", fullxml)


def compare_unparse(obj: dict, **kwargs) -> None:
    try:
        original = xmltodict.unparse(obj, **kwargs)
    except Exception as e:
        with pytest.raises(type(e)):
            xmltodict_rs.unparse(obj, **kwargs)
        return
    rust_impl = xmltodict_rs.unparse(obj, **kwargs)

    if not kwargs.get("full_document", True):
        original_clean = original
        rust_clean = rust_impl
    else:
        original_clean = _strip(original)
        rust_clean = _strip(rust_impl)

    assert rust_clean == original_clean, (
        f"\nObject: {obj!r}\nKwargs: {kwargs}\nOriginal: {original!r}\nRust:     {rust_impl!r}\n"
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


@pytest.mark.parametrize("obj", SIMPLE_OBJECTS)
def test_simple_objects(obj):
    compare_unparse(obj)


@pytest.mark.parametrize("obj", ATTRIBUTE_OBJECTS)
def test_attribute_objects(obj):
    compare_unparse(obj)


@pytest.mark.parametrize("obj", TEXT_CONTENT_OBJECTS)
def test_text_content_objects(obj):
    compare_unparse(obj)


@pytest.mark.parametrize("obj", LIST_OBJECTS)
def test_list_objects(obj):
    compare_unparse(obj)


@pytest.mark.parametrize("obj", NESTED_OBJECTS)
def test_nested_objects(obj):
    compare_unparse(obj)


@pytest.mark.parametrize("obj", SPECIAL_VALUE_OBJECTS)
def test_special_value_objects(obj):
    compare_unparse(obj)


@pytest.mark.parametrize(
    "empty_obj",
    [
        None,
        "",
        [],
        {},
        set(),
        frozenset(),
    ],
)
def test_empty_objects(empty_obj):
    obj = {"a": {"b": empty_obj}}
    compare_unparse(obj)


def test_unparse_accepts_generator_of_strings():
    def gen():
        yield "1"
        yield "2"
        yield ""

    original = xmltodict.unparse({"a": {"b": gen()}})
    rust_impl = xmltodict_rs.unparse({"a": {"b": gen()}})
    assert rust_impl == original


@pytest.mark.parametrize(
    "iterator",
    [
        {1, 2, 3},
        frozenset([1, 2, 3, ""]),
    ],
)
def test_iterators(iterator):
    obj = {"a": {"b": iterator}}

    compare_unparse(obj, short_empty_elements=True)


@pytest.mark.parametrize("obj", SIMPLE_OBJECTS[:3])
def test_short_empty_elements(obj):
    if obj[next(iter(obj.keys()))] is None:
        compare_unparse(obj, short_empty_elements=True)


@pytest.mark.parametrize("obj", SIMPLE_OBJECTS[:3])
def test_full_document_false(obj):
    compare_unparse(obj, full_document=False)


@pytest.mark.parametrize("obj", SIMPLE_OBJECTS[:2])
def test_encoding(obj):
    compare_unparse(obj, encoding="utf-8")


def test_empty_dict():
    with pytest.raises(ValueError):
        xmltodict.unparse({})
    with pytest.raises(ValueError):
        xmltodict_rs.unparse({})


def test_multiple_roots():
    obj = {"a": "1", "b": "2"}
    with pytest.raises(ValueError):
        xmltodict.unparse(obj)
    with pytest.raises(ValueError):
        xmltodict_rs.unparse(obj)


def test_empty_dict_no_full_document():
    compare_unparse({}, full_document=False)


def test_multiple_roots_no_full_document():
    obj = OrderedDict([("a", 1), ("b", 2)])
    compare_unparse(obj, full_document=False)


def test_non_string_values():
    compare_unparse({"a": 1})
    compare_unparse({"a": {"@attr": 1}})


def test_boolean_values():
    compare_unparse({"x": True})
    compare_unparse({"x": False})


def test_pretty_printing():
    obj = {"a": {"b": [{"c": [1, 2]}, 3], "x": "y"}}

    def clean(x: str) -> str:
        return "\n".join(x for x in x.splitlines() if x.strip())

    original = clean(xmltodict.unparse(obj, pretty=True))
    rust_impl = clean(xmltodict_rs.unparse(obj, pretty=True))
    assert original == rust_impl


# Preprocessor tests

PREPROCESSOR_DATA = [
    {"root": {"a": "1", "b": "2"}},
    {"root": {"item": "data"}},
    {"root": {"a": {"@attr": "x"}, "b": "2"}},
    {"root": {"c": {"d": "3"}}},
    {"a": {"b": "inner"}},
    {"root": {"mixed": {"@id": "1", "child": "X"}}},
]


@pytest.mark.parametrize("data", PREPROCESSOR_DATA)
def test_preprocessor_rename_tags(data):
    def pre(key, value):
        return f"new_{key}", value

    compare_unparse(data, preprocessor=pre)


@pytest.mark.parametrize("data", PREPROCESSOR_DATA)
def test_preprocessor_change_values(data):
    def pre(key, value):
        if isinstance(value, str):
            return key, value.upper()
        return key, value

    compare_unparse(data, preprocessor=pre)


@pytest.mark.parametrize("data", PREPROCESSOR_DATA)
def test_preprocessor_skip_element(data):
    def pre(key, value):
        if key == "b":
            return None
        return key, value

    compare_unparse(data, preprocessor=pre)


@pytest.mark.parametrize("data", PREPROCESSOR_DATA)
def test_preprocessor_mixed_changes(data):
    def pre(key, value):
        if key == "a":
            return f"{key}_new", f"{value}_new" if isinstance(value, str) else value
        elif key == "b":
            return None
        return key, value

    compare_unparse(data, preprocessor=pre)


@pytest.mark.parametrize("data", PREPROCESSOR_DATA)
def test_preprocessor_attr_rename(data):
    def pre(key, value):
        if isinstance(value, dict):
            newd = {}
            for k, v in value.items():
                if k.startswith("@"):
                    newd[f"@new_{k[1:]}"] = v
                else:
                    newd[k] = v
            return key, newd
        return key, value

    compare_unparse(data, preprocessor=pre)


@pytest.mark.parametrize("data", PREPROCESSOR_DATA)
def test_preprocessor_attr_value_transform(data):
    def pre(key, value):
        if isinstance(value, dict):
            newd = {}
            for k, v in value.items():
                if k.startswith("@") and isinstance(v, str):
                    newd[k] = v.upper()
                else:
                    newd[k] = v
            return key, newd
        return key, value

    compare_unparse(data, preprocessor=pre)


@pytest.mark.parametrize("data", PREPROCESSOR_DATA)
def test_preprocessor_skip_attribute(data):
    def pre(key, value):
        if isinstance(value, dict):
            newd = {k: v for k, v in value.items() if not (k.startswith("@") and k[1:] == "id")}
            return key, newd
        return key, value

    compare_unparse(data, preprocessor=pre)


@pytest.mark.parametrize("data", PREPROCESSOR_DATA)
def test_preprocessor_raise_exception(data):
    def pre(key, value):
        raise RuntimeError("Pre error")

    compare_unparse(data, preprocessor=pre)


def test_preprocessor_change_none_to_content():
    data = {"root": {"tag": None}}

    def pre(key, value):
        if key == "tag" and value is None:
            return key, "CONTENT"
        return key, value

    compare_unparse(data, preprocessor=pre)


def test_preprocessor_none_to_children():
    data = {"root": {"tag": None}}

    def pre(key, value):
        if key == "tag":
            return key, {"child": "X"}
        return key, value

    compare_unparse(data, preprocessor=pre)


def test_preprocessor_none_to_attr():
    data = {"root": {"tag": None}}

    def pre(key, value):
        if key == "tag":
            return key, {"@id": "123"}
        return key, value

    compare_unparse(data, preprocessor=pre)


# Additional unparse tests


def test_unparse_with_unicode():
    obj = {"root": {"item": "Hello 世界"}}
    compare_unparse(obj)


def test_unparse_with_special_chars():
    obj = {"root": {"item": '<>&"'}}
    compare_unparse(obj)


def test_unparse_deeply_nested():
    obj = {"a": {"b": {"c": {"d": {"e": "deep"}}}}}
    compare_unparse(obj)


def test_unparse_mixed_lists_and_dicts():
    obj = {"root": {"items": {"item": [{"@id": "1", "#text": "a"}, {"@id": "2", "#text": "b"}]}}}
    compare_unparse(obj)


def test_unparse_empty_string():
    obj = {"root": ""}
    compare_unparse(obj)


def test_unparse_none_value():
    obj = {"root": None}
    compare_unparse(obj)


def test_unparse_integer_value():
    obj = {"root": 42}
    compare_unparse(obj)


def test_unparse_float_value():
    obj = {"root": 3.14}
    compare_unparse(obj)
