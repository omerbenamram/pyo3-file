![crates.io](https://img.shields.io/crates/v/pyo3-file.svg)

# PyO3-File

This is a small utility library to facilitate working with python file-like objects with rust.

## Example

An example use case for this is when a file is opened in python, and needs to be passed to a rust library.

We could support both by introspecting the `PyObject`, and pick the correct behavior.

We would like this to work:
```python
from path_or_file_like import accepts_path_or_file_like

def main():
    # should open `some_file.txt`.
    accepts_path_or_file_like("./some_file.txt")

    # should read from the python handle.
    f = open('./some_file.txt')
    accepts_path_or_file_like(f)
```

We could use `pyo3_file` to extend an existing a `pyo3` module.

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

## Type stubs

The optional `experimental-inspect` feature turns on `pyo3/experimental-inspect`, so that a
`PyFileLikeObject` argument is described in PyO3's introspection data and shows up in a generated
`.pyi` as

```python
def accepts_file_like(f: _typeshed.SupportsRead[typing.Any] | _typeshed.SupportsWrite[typing.Any]) -> str: ...
```

instead of the default `_typeshed.Incomplete`, which is `Any` and silently disables checking for
that parameter.

The hint is deliberately structural. `typing.IO` is a plain class rather than a `Protocol`, so type
checkers only accept nominal subclasses of it — which would reject `gzip.GzipFile`, custom
subclasses of `io.RawIOBase`/`io.BufferedIOBase`, and plain objects that just define `.read()`, all
of which work fine here. The `Any` parameter is also intentional: text streams exchange `str` and
binary streams exchange `bytes`, and both are supported.

Because one `FromPyObject` impl is shared by every call site, the hint can only say "supports read
*or* write" — which methods are actually required is a per-call-site decision made through
`py_with_requirements`. A function that knows it only writes can narrow its own stub:

```rust,ignore
#[pyfunction]
#[pyo3(signature = (f: "SupportsWrite[bytes]"))]
fn write_to(f: PyFileLikeObject) -> PyResult<()> { ... }
```

Two caveats there. PyO3 emits such an annotation verbatim as a quoted string and does not generate
an import for the names inside it, so `SupportsWrite` has to be in scope in the generated stub
already. And Python has no intersection type, so "supports read *and* write" cannot be expressed;
neither can seek, since `_typeshed` has no `SupportsSeek`.

### Declaring requirements in the type

`PyFileLike<READ, WRITE, SEEK, FILENO>` puts the four flags that `py_with_requirements` takes into
the type itself, so a single declaration drives the runtime check, the available Rust traits, and
the stub annotation together instead of leaving them to be kept in sync by hand. Aliases are
provided for the usual combinations:

```rust,ignore
use pyo3_file::PySeekableReadFile;

#[pyfunction]
fn parse(mut f: PySeekableReadFile) -> PyResult<Stats> {
    f.seek(SeekFrom::End(-8))?; // `Seek` is available because SEEK is true
    ...
}
```

Extraction rejects an object missing any required method, exactly as `py_with_requirements` would.
Only the requested `std::io` traits are implemented, so calling `read` on a write-only file is a
compile error rather than a runtime `TypeError`. And the stub gets the most precise annotation the
requirement admits:

| requirement | annotation |
| --- | --- |
| read | `_typeshed.SupportsRead[typing.Any]` |
| write | `_typeshed.SupportsWrite[typing.Any]` |
| fileno | `_typeshed.HasFileno` |
| nothing | `_typeshed.SupportsRead[typing.Any] \| _typeshed.SupportsWrite[typing.Any]` |
| anything else | `io.IOBase \| typing.IO[typing.Any]` |

The last row is the limit of what Python's type system can state: there is no intersection type, so
"supports read *and* write" is not expressible, and `_typeshed` has no `SupportsSeek`. Those cases
fall back to a union of the two nominal file base types, which together cover every file object in
the standard library — `io.IOBase` catches `gzip.GzipFile` and custom `RawIOBase`/`BufferedIOBase`
subclasses that `typing.IO` rejects, and `typing.IO` catches `tempfile.NamedTemporaryFile`, which
`io.IOBase` rejects. Being nominal, neither accepts a plain duck-typed object, so those combinations
are less permissive than the crate is at runtime.
