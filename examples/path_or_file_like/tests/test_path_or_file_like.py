import io
from pathlib import Path

import pytest
import path_or_file_like
from path_or_file_like import (
    accepts_file_like_write,
    accepts_path_or_file_like_read,
    name_of_file_like,
    read_exact_in_steps,
    write_buffered,
)


@pytest.fixture
def small_sample() -> str:
    p = Path(__file__).parent.parent
    return str(p.joinpath("sample.txt"))


def test_it_reads_from_io_object(small_sample):
    with open(small_sample, "rb") as o:
        r = o.read()

    assert accepts_path_or_file_like_read(io.BytesIO(r)) == "Hello World!"


def test_it_reads_from_textio_object(small_sample):
    with open(small_sample, "rt") as o:
        r = o.read()

    assert accepts_path_or_file_like_read(io.StringIO(r)) == "Hello World!"


def test_it_reads_from_file_backed_object(small_sample):
    with open(small_sample, "rb") as o:
        assert accepts_path_or_file_like_read(o) == "Hello World!"


def test_it_fails_on_non_file_object():
    with pytest.raises(TypeError):
        accepts_path_or_file_like_read(3)


def test_it_fails_when_write_returns_none():
    class FileLike:
        def write(self, _data):
            return None

    with pytest.raises(
        OSError, match=r"write\(\) returned None, expected number of bytes written"
    ):
        accepts_file_like_write(FileLike())


def test_it_writes_to_io_object():
    f = io.BytesIO()
    accepts_file_like_write(f)
    assert f.getvalue() == b"Hello, world!"


def test_it_writes_to_textio_object():
    f = io.StringIO()
    accepts_file_like_write(f)
    assert f.getvalue() == "Hello, world!"


def test_file_name_from_path(small_sample: str):
    with open(small_sample, "rb") as o:
        assert name_of_file_like(o) == small_sample


def test_file_name_from_textio():
    f = io.StringIO()
    assert name_of_file_like(f) is None


TEXT = "h\u00e9llo, \u65e5\u672c\u8a9e \U0001f600 w\u00f6rld"


@pytest.mark.parametrize("buffer_size", [0, 1, 2, 3, 4, 5, 7, 64])
def test_it_writes_non_ascii_text_wherever_the_buffer_splits_it(buffer_size):
    # A buffered writer hands bytes on at any offset, inside a character's encoding too.
    f = io.StringIO()
    write_buffered(f, TEXT.encode(), buffer_size)
    assert f.getvalue() == TEXT


def test_it_writes_non_ascii_bytes_to_a_binary_object():
    f = io.BytesIO()
    write_buffered(f, TEXT.encode(), 3)
    assert f.getvalue() == TEXT.encode()


@pytest.mark.parametrize("data", [b"\xff", b"ok \xff more", b"\xe6\x97 x"])
def test_bytes_that_are_not_utf8_are_an_error_for_a_text_object(data):
    with pytest.raises(OSError, match="not UTF-8"):
        write_buffered(io.StringIO(), data, 0)


def test_text_that_ends_inside_a_character_is_an_error_at_flush():
    f = io.StringIO()
    with pytest.raises(OSError, match="end inside a character"):
        write_buffered(f, b"abc\xe6\x97", 0)
    assert f.getvalue() == "abc"


def test_a_failed_text_write_can_be_retried():
    class Flaky(io.StringIO):
        failed = False

        def write(self, text):
            # Fails once, on the write that completes the character split before it.
            if "\u65e5" in text and not self.failed:
                self.failed = True
                raise InterruptedError
            return super().write(text)

    f = Flaky()
    # `write_all` writes again after an interruption.
    write_buffered(f, "a\u65e5b".encode(), 2)
    assert f.failed
    assert f.getvalue() == "a\u65e5b"


def test_it_fails_when_a_text_write_is_short():
    class Short(io.StringIO):
        def write(self, text):
            return super().write(text[:1])

    with pytest.raises(OSError, match=r"write\(\) of 13 characters returned 1"):
        accepts_file_like_write(Short())


def test_it_fails_when_write_returns_more_than_it_was_given():
    class FileLike:
        def write(self, _data):
            return 100

    with pytest.raises(OSError, match=r"write\(\) of 13 bytes returned 100"):
        accepts_file_like_write(FileLike())


@pytest.mark.parametrize("step", [1, 2, 3, 4, 5, 1000])
def test_it_reads_text_into_buffers_of_any_size(step):
    data = TEXT.encode()
    assert read_exact_in_steps(io.StringIO(TEXT), len(data), step) == data


def test_it_fails_when_read_returns_more_than_asked():
    class FileLike:
        def read(self, _size):
            return b"12345678"

    with pytest.raises(OSError, match=r"read\(4\) returned 8 bytes"):
        read_exact_in_steps(FileLike(), 4, 4)


unix_only = pytest.mark.skipif(
    not hasattr(path_or_file_like, "raw_fd"), reason="file descriptors are Unix only"
)
# `raw_fd` requires a descriptor when it wraps the object; `try_raw_fd` asks for it later.
fd_function = pytest.mark.parametrize("fd", ["raw_fd", "try_raw_fd"])


@unix_only
@fd_function
def test_it_gives_the_descriptor_of_a_real_file(small_sample, fd):
    for mode in ["rb", "rt"]:
        with open(small_sample, mode) as o:
            assert getattr(path_or_file_like, fd)(o) == o.fileno()


@unix_only
@fd_function
@pytest.mark.parametrize("make", [lambda: io.BytesIO(b"data"), lambda: io.StringIO("text")])
def test_an_object_without_a_descriptor_is_an_error(make, fd):
    # Asking such an object for its descriptor used to panic.
    with pytest.raises(io.UnsupportedOperation, match="fileno"):
        getattr(path_or_file_like, fd)(make())


@unix_only
@fd_function
def test_a_closed_file_is_an_error(small_sample, fd):
    o = open(small_sample, "rb")
    o.close()
    with pytest.raises(ValueError, match="closed file"):
        getattr(path_or_file_like, fd)(o)


@unix_only
@pytest.mark.parametrize(
    "fd, error", [("raw_fd", TypeError), ("try_raw_fd", AttributeError)]
)
def test_an_object_without_fileno_is_an_error(fd, error):
    class FileLike:
        def read(self, _size):
            return b""

    with pytest.raises(error, match="fileno"):
        getattr(path_or_file_like, fd)(FileLike())


@unix_only
@fd_function
@pytest.mark.parametrize("value", [-1, -7, 2**40, "3", None, 1.5, True])
def test_a_fileno_that_is_not_a_descriptor_is_an_error(value, fd):
    class FileLike:
        def fileno(self):
            return value

    with pytest.raises(OSError, match=r"fileno\(\) returned .*, expected a file descriptor"):
        getattr(path_or_file_like, fd)(FileLike())
