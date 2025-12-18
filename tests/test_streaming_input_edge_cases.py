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


class BytesSubclass(bytes):
    pass


def test_generator_bytearray_chunk():
    """Generator yielding bytearray chunks should be parsed correctly."""
    result = xmltodict_rs.parse(chunk for chunk in [bytearray(b"<root/>")])
    assert result == {"root": None}


def test_generator_memoryview_chunk():
    """Generator yielding memoryview chunks should be parsed correctly."""
    result = xmltodict_rs.parse(chunk for chunk in [memoryview(b"<root/>")])
    assert result == {"root": None}


def test_generator_bytes_subclass_chunk():
    """Generator yielding bytes subclass chunks should be parsed correctly."""
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


@pytest.mark.parametrize("message", ["a|b", "ошибка 世界"])
def test_generator_exception_message_roundtrips(message):
    def gen():
        yield b"<root>"
        raise ValueError(message)

    assert_parse_parity(gen, match_message=True)


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


def test_file_like_splits_utf8_multibyte_across_reads():
    xml = "<root>世界</root>".encode()
    split_pos = xml.index("世".encode()) + 1
    assert_parse_parity(lambda: ReadFromChunks([xml[:split_pos], xml[split_pos:]]))


# === Additional edge cases for complete coverage ===


def test_generator_mixed_bytes_and_str():
    """Generator yielding mixed bytes and str chunks should work."""

    def gen():
        yield b"<root>"
        yield "<item>1</item>"
        yield b"</root>"

    assert_parse_parity(gen)


def test_generator_empty():
    """Empty generator should raise parse error."""

    def gen():
        return
        yield  # make it a generator

    assert_parse_parity(gen)


def test_generator_only_empty_chunks():
    """Generator yielding only empty chunks should raise parse error."""

    def gen():
        yield b""
        yield b""
        yield b""

    assert_parse_parity(gen)


def test_generator_empty_str_chunks():
    """Generator yielding empty string chunks should be ignored."""

    def gen():
        yield "<root>"
        yield ""
        yield "</root>"

    assert_parse_parity(gen)


def test_file_like_empty():
    """Empty file-like should raise parse error."""

    class EmptyFileLike:
        def read(self, size: int = -1) -> bytes:
            return b""

    assert_parse_parity(EmptyFileLike)


class CustomError(Exception):
    """Custom exception for testing."""

    pass


def test_generator_custom_exception_preserved():
    """Custom exception type should be preserved from generator."""

    def gen():
        yield b"<root>"
        raise CustomError("custom message")

    assert_parse_parity(gen, match_message=True)


def test_file_like_custom_exception_preserved():
    """Custom exception type should be preserved from file-like read()."""

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
    """Various exception types should be preserved from generator."""

    def gen(et=exc_type):
        yield b"<root>"
        raise et("test message")

    assert_parse_parity(gen, match_message=True)


@pytest.mark.parametrize(
    "exc_type",
    [ValueError, TypeError, RuntimeError, KeyError, OSError, AttributeError],
)
def test_file_like_exception_types_preserved(exc_type):
    """Various exception types should be preserved from file-like read()."""

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
    """Exception with multiple args should be preserved."""

    def gen():
        yield b"<root>"
        raise ValueError("arg1", "arg2", 123)

    # Check that exception args are preserved
    try:
        xmltodict.parse(gen())
    except ValueError as e:
        expected_args = e.args

    try:
        xmltodict_rs.parse(gen())
    except ValueError as e:
        assert e.args == expected_args
