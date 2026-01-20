import io

import pytest
import xmltodict

import xmltodict_rs


def assert_parse_parity(factory, *, match_message: bool = False) -> None:
    try:
        expected = xmltodict.parse(factory())
    except Exception as expected_err:
        with pytest.raises(type(expected_err)) as actual_err:
            xmltodict_rs.parse(factory())
        if match_message:
            assert str(actual_err.value) == str(expected_err)
        return
    actual = xmltodict_rs.parse(factory())
    assert actual == expected


# Basic input types


def test_parse_string():
    xml = "<root><item>test</item></root>"
    result = xmltodict_rs.parse(xml)
    assert result == {"root": {"item": "test"}}


def test_parse_bytes():
    xml = b"<root><item>test</item></root>"
    result = xmltodict_rs.parse(xml)
    assert result == {"root": {"item": "test"}}


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


def test_parse_invalid_input_type_raises():
    with pytest.raises(TypeError):
        xmltodict_rs.parse(123)


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


# Generator input


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


# Generator edge cases


class BytesSubclass(bytes):
    pass


def test_generator_bytearray_chunk():
    result = xmltodict_rs.parse(chunk for chunk in [bytearray(b"<root/>")])
    assert result == {"root": None}


def test_generator_memoryview_chunk():
    result = xmltodict_rs.parse(chunk for chunk in [memoryview(b"<root/>")])
    assert result == {"root": None}


def test_generator_bytes_subclass_chunk():
    result = xmltodict_rs.parse(chunk for chunk in [BytesSubclass(b"<root/>")])
    assert result == {"root": None}


def test_generator_many_empty_chunks_are_ignored():
    def gen():
        yield b"<root>"
        for _ in range(1000):
            yield b""
        yield b"</root>"

    assert_parse_parity(gen)


def test_generator_single_byte_chunks():
    xml = b"<root><a>1</a><b>2</b><c>3</c></root>"

    def gen():
        for byte in xml:
            yield bytes([byte])

    assert_parse_parity(gen)


def test_generator_splits_utf8_multibyte_across_chunks():
    xml = "<root>世界</root>".encode()
    split_pos = xml.index("世".encode()) + 1

    def gen():
        yield xml[:split_pos]
        yield xml[split_pos:]

    assert_parse_parity(gen)


@pytest.mark.parametrize(
    "factory",
    [
        lambda: (chunk for chunk in [b"<root>", object()]),
        lambda: (chunk for chunk in [b"<root>", 1.23]),
        lambda: (chunk for chunk in [b"<root>", []]),
    ],
)
def test_generator_invalid_chunk_types(factory):
    assert_parse_parity(factory)


def test_generator_mixed_bytes_and_str():
    def gen():
        yield b"<root>"
        yield "<item>1</item>"
        yield b"</root>"

    assert_parse_parity(gen)


def test_generator_empty():
    def gen():
        return
        yield

    assert_parse_parity(gen)


def test_generator_only_empty_chunks():
    def gen():
        yield b""
        yield b""
        yield b""

    assert_parse_parity(gen)


def test_generator_empty_str_chunks():
    def gen():
        yield "<root>"
        yield ""
        yield "</root>"

    assert_parse_parity(gen)


# File-like edge cases


class ReadFromChunks:
    def __init__(self, chunks) -> None:
        self._chunks = list(chunks)
        self._index = 0

    def read(self, size: int = -1):
        if self._index >= len(self._chunks):
            return b""
        chunk = self._chunks[self._index]
        self._index += 1
        return chunk


@pytest.mark.parametrize(
    "chunks",
    [
        [bytearray(b"<root/>")],
        [memoryview(b"<root/>")],
        [BytesSubclass(b"<root/>")],
    ],
)
def test_file_like_read_bytes_like_variants(chunks):
    assert_parse_parity(lambda: ReadFromChunks(chunks))


class OversizeReader:
    def __init__(self, data: bytes, extra: int) -> None:
        self._data = data
        self._extra = extra
        self._offset = 0

    def read(self, size: int = -1) -> bytes:
        if self._offset >= len(self._data):
            return b""
        if size < 0:
            end = len(self._data)
        else:
            end = min(self._offset + size + self._extra, len(self._data))
        chunk = self._data[self._offset : end]
        self._offset = end
        return chunk


def test_file_like_read_ignores_size_and_returns_oversized_chunks():
    xml = b"<root><a>1</a><b>2</b><c>3</c></root>"
    assert_parse_parity(lambda: OversizeReader(xml, extra=128))


def test_file_like_read_returns_partial_then_rest():
    xml = b"<root><a>1</a><b>2</b><c>3</c></root>"
    assert_parse_parity(lambda: OversizeReader(xml, extra=0))


def test_file_like_premature_empty_chunk_causes_parse_error():
    assert_parse_parity(lambda: ReadFromChunks([b"<root>", b""]))


@pytest.mark.parametrize("bad_chunk", [None, 123])
def test_file_like_read_bad_chunk_types(bad_chunk):
    assert_parse_parity(lambda: ReadFromChunks([b"<root>", bad_chunk]))


def test_file_like_splits_utf8_multibyte_across_reads():
    xml = "<root>世界</root>".encode()
    split_pos = xml.index("世".encode()) + 1
    assert_parse_parity(lambda: ReadFromChunks([xml[:split_pos], xml[split_pos:]]))


def test_file_like_empty():
    class EmptyFileLike:
        def read(self, size: int = -1) -> bytes:
            return b""

    assert_parse_parity(EmptyFileLike)


# Error propagation


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


@pytest.mark.parametrize("message", ["a|b", "ошибка 世界"])
def test_generator_exception_message_roundtrips(message):
    def gen():
        yield b"<root>"
        raise ValueError(message)

    assert_parse_parity(gen, match_message=True)


@pytest.mark.parametrize("message", ["a|b", "ошибка 世界"])
def test_file_like_read_exception_message_roundtrips(message):
    class FileLike:
        def __init__(self) -> None:
            self._called = 0

        def read(self, size: int = -1) -> bytes:
            self._called += 1
            if self._called == 1:
                return b"<root>"
            raise RuntimeError(message)

    assert_parse_parity(FileLike, match_message=True)


def test_file_like_read_raises_stop_iteration():
    class FileLike:
        def __init__(self) -> None:
            self._called = 0

        def read(self, size: int = -1) -> bytes:
            self._called += 1
            if self._called == 1:
                return b"<root>"
            raise StopIteration("boom")

    assert_parse_parity(FileLike)


class CustomError(Exception):
    pass


def test_generator_custom_exception_preserved():
    def gen():
        yield b"<root>"
        raise CustomError("custom message")

    assert_parse_parity(gen, match_message=True)


def test_file_like_custom_exception_preserved():
    class FileLike:
        def __init__(self) -> None:
            self._called = 0

        def read(self, size: int = -1) -> bytes:
            self._called += 1
            if self._called == 1:
                return b"<root>"
            raise CustomError("custom message")

    assert_parse_parity(FileLike, match_message=True)


@pytest.mark.parametrize(
    "exc_type",
    [ValueError, TypeError, RuntimeError, KeyError, OSError, AttributeError],
)
def test_generator_exception_types_preserved(exc_type):
    def gen(et=exc_type):
        yield b"<root>"
        raise et("test message")

    assert_parse_parity(gen, match_message=True)


@pytest.mark.parametrize(
    "exc_type",
    [ValueError, TypeError, RuntimeError, KeyError, OSError, AttributeError],
)
def test_file_like_exception_types_preserved(exc_type):
    class FileLike:
        def __init__(self, et) -> None:
            self._called = 0
            self._exc_type = et

        def read(self, size: int = -1) -> bytes:
            self._called += 1
            if self._called == 1:
                return b"<root>"
            raise self._exc_type("test message")

    assert_parse_parity(lambda: FileLike(exc_type), match_message=True)


def test_generator_exception_with_multiple_args():
    def gen():
        yield b"<root>"
        raise ValueError("arg1", "arg2", 123)

    try:
        xmltodict.parse(gen())
    except ValueError as e:
        expected_args = e.args

    try:
        xmltodict_rs.parse(gen())
    except ValueError as e:
        assert e.args == expected_args
