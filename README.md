# pyo3-file

[![crates.io](https://img.shields.io/crates/v/pyo3-file.svg)](https://crates.io/crates/pyo3-file)
[![docs.rs](https://img.shields.io/badge/docs-latest-blue.svg)](https://docs.rs/pyo3-file)

`pyo3-file` lets Rust code read, write and seek a Python file-like object. `PyFileLikeObject` wraps any Python object with `read`, `write` or `seek` methods and implements `std::io::Read`, `Write` and `Seek` for it, so the object can go wherever a `File` can.

Use it when a [PyO3](https://pyo3.rs) extension should accept an open file, an `io.BytesIO` or a stream from a zip or a bucket, and not only a path.

## Example

A function that takes either a path or a file-like object, so that both of these work from Python:

```python
from path_or_file_like import accepts_path_or_file_like

def main():
    # Opens `some_file.txt` itself.
    accepts_path_or_file_like("./some_file.txt")

    # Reads from the Python file object.
    f = open('./some_file.txt')
    accepts_path_or_file_like(f)
```

The Rust side tries the argument as a path first, and wraps it in a `PyFileLikeObject` otherwise:

```rust
use pyo3_file::PyFileLikeObject;

use pyo3::prelude::*;
use pyo3::wrap_pyfunction;
use std::io::Read;
use std::fs::File;

/// Represents either a path `File` or a file-like object `FileLike`
#[derive(Debug)]
enum FileOrFileLike {
    File(String),
    FileLike(PyFileLikeObject),
}

impl<'py> FromPyObject<'_, 'py> for FileOrFileLike {
    type Error = PyErr;

    fn extract(obj: Borrowed<'_, 'py, PyAny>) -> Result<Self, Self::Error> {
        // is a path
        if let Ok(string) = obj.extract::<String>() {
            return Ok(FileOrFileLike::File(string));
        }

        // is a file-like
        let f =
            PyFileLikeObject::py_with_requirements(obj.as_any().clone(), true, false, true, false)?;
        Ok(FileOrFileLike::FileLike(f))
    }
}

#[pyfunction]
/// Opens a file or file-like, and reads it to string.
fn accepts_path_or_file_like(path_or_file_like: FileOrFileLike) -> PyResult<String> {
    match path_or_file_like {
        FileOrFileLike::File(s) => {
            println!("It's a file! - path {}", s);
            let mut f = File::open(s)?;
            let mut string = String::new();

            f.read_to_string(&mut string)?;
            Ok(string)
        }
        FileOrFileLike::FileLike(mut f) => {
            println!("Its a file-like object");
            let mut string = String::new();

            f.read_to_string(&mut string)?;
            Ok(string)
        }
    }
}

#[pymodule]
fn path_or_file_like(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_wrapped(wrap_pyfunction!(accepts_path_or_file_like))?;

    Ok(())
}


# fn main() {}
```

`py_with_requirements(object, read, write, seek, fileno)` checks when the object is wrapped that it has the methods you will need, so a wrong argument is a `TypeError` at the call and not an I/O error in the middle of a read. The complete extension, with its tests, is in [`examples/path_or_file_like`](examples/path_or_file_like).

A Python object can raise from any of its methods or return something unexpected. Every such failure comes back as an `io::Error`, not a panic. The one exception is `AsRawFd::as_raw_fd`, which has no way to return an error: `try_as_raw_fd` is the form that does.

## Type stubs

The optional `experimental-inspect` feature turns on `pyo3/experimental-inspect`, so that a
`PyFileLikeObject` argument is described in PyO3's introspection data and shows up in a generated
`.pyi` as

```python
def accepts_file_like(f: _typeshed.SupportsRead[typing.Any] | _typeshed.SupportsWrite[typing.Any]) -> str: ...
```

One `FromPyObject` impl is shared by every call site, so the hint can only say "supports read
*or* write". Which methods are required is decided at each call site, through
`py_with_requirements`. A function that knows it only writes can narrow its own stub:

```rust,ignore
#[pyfunction]
#[pyo3(signature = (f: "SupportsWrite[bytes]"))]
fn write_to(f: PyFileLikeObject) -> PyResult<()> { ... }
```
