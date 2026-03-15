import io
from typing import Any

import pytest
import xmltodict

import xmltodict_rs

# Non-ASCII test strings for encodings that don't share byte representation with UTF-8.
# Windows-1251 requires Cyrillic characters to actually test decoding.
CYRILLIC_SAMPLE = "\u041f\u0440\u0438\u0432\u0435\u0442"
LATIN_ACCENTED = "caf\xe9"


def compare_parse(xml_input: bytes, **kwargs: Any) -> None:
    expected = xmltodict.parse(xml_input, **kwargs)
    actual = xmltodict_rs.parse(xml_input, **kwargs)
    assert actual == expected


# --- parse: auto-detection from BOM ---


def test_parse_utf8_bytes_with_bom() -> None:
    compare_parse(b"\xef\xbb\xbf<root><item>Hello</item></root>")


def test_parse_utf16_le_bom_auto_detect() -> None:
    xml_str = '<?xml version="1.0" encoding="utf-16"?>\n<root><item>world</item></root>'
    xml_bytes = b"\xff\xfe" + xml_str.encode("utf-16-le")
    result = xmltodict_rs.parse(xml_bytes)
    assert result == {"root": {"item": "world"}}


def test_parse_utf16_be_bom_auto_detect() -> None:
    xml_str = '<?xml version="1.0" encoding="utf-16"?>\n<root><item>world</item></root>'
    xml_bytes = b"\xfe\xff" + xml_str.encode("utf-16-be")
    result = xmltodict_rs.parse(xml_bytes)
    assert result == {"root": {"item": "world"}}


def test_parse_utf16_le_no_bom_auto_detect() -> None:
    xml_bytes = "<root><item>Hi</item></root>".encode("utf-16-le")
    result = xmltodict_rs.parse(xml_bytes)
    assert result == {"root": {"item": "Hi"}}


def test_parse_utf16_be_no_bom_auto_detect() -> None:
    xml_bytes = "<root><item>Hi</item></root>".encode("utf-16-be")
    result = xmltodict_rs.parse(xml_bytes)
    assert result == {"root": {"item": "Hi"}}


# --- parse: auto-detection from XML declaration ---


def test_parse_windows_1251_from_declaration() -> None:
    xml_str = (
        '<?xml version="1.0" encoding="windows-1251"?>\n'
        f"<root><item>{CYRILLIC_SAMPLE}</item></root>"
    )
    xml_bytes = xml_str.encode("windows-1251")
    expected = xmltodict.parse(xml_bytes)
    actual = xmltodict_rs.parse(xml_bytes)
    assert actual == expected == {"root": {"item": CYRILLIC_SAMPLE}}


def test_parse_iso_8859_1_from_declaration() -> None:
    xml_str = (
        f'<?xml version="1.0" encoding="iso-8859-1"?>\n<root><item>{LATIN_ACCENTED}</item></root>'
    )
    xml_bytes = xml_str.encode("iso-8859-1")
    expected = xmltodict.parse(xml_bytes)
    actual = xmltodict_rs.parse(xml_bytes)
    assert actual == expected == {"root": {"item": LATIN_ACCENTED}}


# --- parse: explicit encoding ---


@pytest.mark.parametrize(
    ("encoding", "text"),
    [
        ("utf-16-le", "Hello world"),
        ("utf-16-be", "Hello world"),
        ("utf-16-le", "Hello \U0001f30d"),
        ("windows-1251", CYRILLIC_SAMPLE),
        ("iso-8859-1", LATIN_ACCENTED),
        ("cp1251", CYRILLIC_SAMPLE),
    ],
)
def test_parse_explicit_encoding(encoding: str, text: str) -> None:
    xml_str = f"<root><item>{text}</item></root>"
    py_encoding = encoding.replace("cp1251", "windows-1251")
    xml_bytes = xml_str.encode(py_encoding)
    result = xmltodict_rs.parse(xml_bytes, encoding=encoding)
    assert result == {"root": {"item": text}}


def test_parse_attributes_non_utf8() -> None:
    xml_str = f'<root id="{CYRILLIC_SAMPLE}"><item>data</item></root>'
    xml_bytes = xml_str.encode("windows-1251")
    result = xmltodict_rs.parse(xml_bytes, encoding="windows-1251")
    assert result == {"root": {"@id": CYRILLIC_SAMPLE, "item": "data"}}


# --- parse: file-like with encoding ---


def test_parse_file_like_with_encoding() -> None:
    xml_str = f"<root><item>{CYRILLIC_SAMPLE}</item></root>"
    xml_bytes = xml_str.encode("windows-1251")
    result = xmltodict_rs.parse(io.BytesIO(xml_bytes), encoding="windows-1251")
    assert result == {"root": {"item": CYRILLIC_SAMPLE}}


# --- parse: generator with encoding ---


def test_parse_generator_with_encoding() -> None:
    xml_str = f"<root><item>{CYRILLIC_SAMPLE}</item></root>"
    xml_bytes = xml_str.encode("windows-1251")

    def gen():
        yield xml_bytes[:10]
        yield xml_bytes[10:]

    result = xmltodict_rs.parse(gen(), encoding="windows-1251")
    assert result == {"root": {"item": CYRILLIC_SAMPLE}}


# --- parse: error cases ---


def test_parse_unknown_encoding_raises() -> None:
    with pytest.raises(LookupError):
        xmltodict_rs.parse(b"<root/>", encoding="nonexistent-encoding")


# --- unparse: always returns str (matches original xmltodict) ---


@pytest.mark.parametrize(
    ("encoding", "expected_label"),
    [
        ("utf-8", "utf-8"),
        ("windows-1251", "windows-1251"),
        ("utf-16le", "utf-16le"),
        ("iso-8859-1", "iso-8859-1"),
    ],
)
def test_unparse_returns_str(encoding: str, expected_label: str) -> None:
    data = {"root": {"item": "Hello"}}
    result = xmltodict_rs.unparse(data, encoding=encoding)
    assert isinstance(result, str)
    assert f'encoding="{expected_label}"' in result
    assert "Hello" in result


# --- roundtrip: parse(unparse().encode(enc), encoding=enc) ---


@pytest.mark.parametrize(
    ("encoding", "py_encoding", "text"),
    [
        ("windows-1251", "windows-1251", CYRILLIC_SAMPLE),
        ("iso-8859-1", "iso-8859-1", LATIN_ACCENTED),
    ],
)
def test_roundtrip(encoding: str, py_encoding: str, text: str) -> None:
    original = {"root": {"item": text}}
    xml_str = xmltodict_rs.unparse(original, encoding=encoding)
    xml_bytes = xml_str.encode(py_encoding)
    result = xmltodict_rs.parse(xml_bytes, encoding=encoding)
    assert result == original


def test_roundtrip_auto_detect_from_declaration() -> None:
    original = {"root": {"item": CYRILLIC_SAMPLE}}
    xml_str = xmltodict_rs.unparse(original, encoding="windows-1251")
    xml_bytes = xml_str.encode("windows-1251")
    result = xmltodict_rs.parse(xml_bytes)
    assert result == original
