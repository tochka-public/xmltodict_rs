import io

import pytest
import xmltodict

import xmltodict_rs


def test_parse_accepts_file_like_text():
    xml = "<root><item>1</item><item>2</item></root>"
    with pytest.raises(TypeError):
        xmltodict.parse(io.StringIO(xml))
    with pytest.raises(TypeError):
        xmltodict_rs.parse(io.StringIO(xml))


def test_parse_accepts_file_like_bytes():
    xml = b"<root><item>1</item><item>2</item></root>"
    original = xmltodict.parse(io.BytesIO(xml))
    rust_impl = xmltodict_rs.parse(io.BytesIO(xml))
    assert rust_impl == original


def test_parse_accepts_generator_of_strings():
    def gen():
        yield "<root>"
        yield "<item>1</item>"
        yield ""
        yield "<item>2</item>"
        yield "</root>"

    original = xmltodict.parse(gen())
    rust_impl = xmltodict_rs.parse(gen())
    assert rust_impl == original


def test_parse_accepts_generator_of_bytes():
    def gen():
        yield b"<root>"
        yield b"<item>1</item>"
        yield b""
        yield b"<item>2</item>"
        yield b"</root>"

    original = xmltodict.parse(gen())
    rust_impl = xmltodict_rs.parse(gen())
    assert rust_impl == original


def test_parse_accepts_iterable_of_strings():
    chunks = ["<root>", "<item>1</item>", "<item>2</item>", "</root>"]
    with pytest.raises(TypeError):
        xmltodict.parse(iter(chunks))
    with pytest.raises(TypeError):
        xmltodict_rs.parse(iter(chunks))


def test_parse_accepts_file_like_without_sized_read():
    class FileLike:
        def __init__(self, data: bytes) -> None:
            self._data = data
            self._used = False

        def read(self) -> bytes:
            if self._used:
                return b""
            self._used = True
            return self._data

    xml = b"<root><item>1</item></root>"
    with pytest.raises(TypeError):
        xmltodict.parse(FileLike(xml))
    with pytest.raises(TypeError):
        xmltodict_rs.parse(FileLike(xml))


def test_parse_invalid_input_type_raises():
    with pytest.raises(TypeError):
        xmltodict_rs.parse(123)
