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
use pyo3::types::PyString;

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

impl FileOrFileLike {
    pub fn from_pyobject(path_or_file_like: PyObject) -> PyResult<FileOrFileLike> {
        Python::with_gil(|py| {
            // is a path
            if let Ok(string_ref) = path_or_file_like.downcast::<PyString>(py) {
                return Ok(FileOrFileLike::File(
                    string_ref.to_string_lossy().to_string(),
                ));
            }

            // is a file-like
            match PyFileLikeObject::with_requirements(path_or_file_like, true, false, true, false) {
                Ok(f) => Ok(FileOrFileLike::FileLike(f)),
                Err(e) => Err(e)
            }
        })
    }
}

#[pyfunction]
/// Opens a file or file-like, and reads it to string.
fn accepts_path_or_file_like(
    path_or_file_like: PyObject,
) -> PyResult<String> {
    Python::with_gil(|py| {
        match FileOrFileLike::from_pyobject(path_or_file_like) {
            Ok(f) => match f {
                FileOrFileLike::File(s) => {
                    println!("It's a file! - path {}", s);
                    let mut f = File::open(s)?;
                    let mut string = String::new();

                    let read = f.read_to_string(&mut string);
                    Ok(string)
                }
                FileOrFileLike::FileLike(mut f) => {
                    println!("Its a file-like object");
                    let mut string = String::new();

                    let read = f.read_to_string(&mut string);
                    Ok(string)
                }
            },
            Err(e) => Err(e),
        }
    })
}

#[pymodule]
fn path_or_file_like(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_wrapped(wrap_pyfunction!(accepts_path_or_file_like))?;

    Ok(())
}


# fn main() {}
```
