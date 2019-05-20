import pytest
import io
from pathlib import Path

from path_or_file_like import accepts_path_or_file_like


@pytest.fixture
def small_sample() -> str:
    p = Path(__file__).parent.parent
    return str(p.joinpath('sample.txt'))


def test_it_works_on_io_object(small_sample):
    with open(small_sample, "rb") as o:
        r = o.read()

    assert accepts_path_or_file_like(io.BytesIO(r)) == "Hello World!"


def test_it_works_on_file_backed_object(small_sample):
    with open(small_sample, "rb") as o:
        assert accepts_path_or_file_like(o) == "Hello World!"


def test_it_fails_on_non_file_object():
    with pytest.raises(TypeError):
        accepts_path_or_file_like(3)

