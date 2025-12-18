import io

import pytest
import xmltodict

import xmltodict_rs


def test_generator_exception_propagates_as_same_type_and_message():
    def gen():
        yield b"<root>"
        raise ValueError("boom")

    with pytest.raises(ValueError, match=r"^boom$"):
        xmltodict_rs.parse(gen())


def test_generator_yields_none_becomes_type_error():
    def gen():
        yield b"<root>"
        yield None

    with pytest.raises(TypeError, match=r"NoneType"):
        xmltodict_rs.parse(gen())


def test_generator_yields_int_becomes_type_error():
    def gen():
        yield b"<root>"
        yield 123

    with pytest.raises(TypeError):
        xmltodict_rs.parse(gen())


def test_file_like_read_exception_propagates_as_same_type_and_message():
    class FileLike:
        def __init__(self) -> None:
            self._called = 0

        def read(self, size: int = -1) -> bytes:
            self._called += 1
            if self._called == 1:
                return b"<root>"
            raise RuntimeError("boom")

    with pytest.raises(RuntimeError, match=r"^boom$"):
        xmltodict_rs.parse(FileLike())


def test_file_like_read_returning_str_becomes_type_error_with_type_name():
    with pytest.raises(TypeError, match=r"type=str"):
        xmltodict_rs.parse(io.StringIO("<root/>"))


def test_generator_invalid_xml_still_reports_xml_parse_error():
    def gen():
        yield b"<<invalid>>"
        yield b""

    try:
        xmltodict.parse(gen())
    except Exception as expected_err:
        with pytest.raises(type(expected_err)):
            xmltodict_rs.parse(gen())
    else:
        pytest.fail("Expected xmltodict.parse to raise")
