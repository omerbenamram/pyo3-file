import io
from pathlib import Path

import pytest
from path_or_file_like import (
    accepts_file_like_write,
    accepts_path_or_file_like_read,
    name_of_file_like,
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
