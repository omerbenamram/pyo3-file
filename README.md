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
use std::path::PathBuf;

/// Represents either a path `File` or a file-like object `FileLike`
#[derive(Debug, FromPyObject)]
enum FileOrFileLike {
    #[pyo3(annotation = "str")]
    FileName(PathBuf),
    #[pyo3(annotation = "file-like")]
    FileLike(
        #[pyo3(from_py_with = "PyFileLikeObject::with_rw_seek::<true, false, true>")]
        PyFileLikeObject
    ),
}

#[pyfunction]
/// Opens a file or file-like, and reads it to string.
fn accepts_path_or_file_like(
    f: FileOrFileLike,
) -> PyResult<String> {
    match f {
        FileOrFileLike::FileName(s) => {
            println!("It's a file! - path {}", s.display());
            let mut f = File::open(s)?;
            let mut string = String::new();

            let _read = f.read_to_string(&mut string);
            Ok(string)
        }
        FileOrFileLike::FileLike(mut f) => {
            println!("Its a file-like object");
            let mut string = String::new();

            let _read = f.read_to_string(&mut string);
            Ok(string)
        }
    }
}

#[pymodule]
fn path_or_file_like(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_wrapped(wrap_pyfunction!(accepts_path_or_file_like))?;

    Ok(())
}


# fn main() {}
```
