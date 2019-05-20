# PyO3-File

This a small utility library to facilitate working with python file-like with rust.

## Example

```rust
use pyo3_file::PyFileLikeObject;
use pyo3::types::PyString;

use pyo3::prelude::*;
use pyo3::wrap_pyfunction;
use std::io::Read;

/// Represents either a path `File` or a file-like object `FileLike`
#[derive(Debug)]
enum FileOrFileLike {
    File(String),
    FileLike(PyFileLikeObject),
}

impl FileOrFileLike {
    pub fn from_pyobject(path_or_file_like: PyObject) -> PyResult<FileOrFileLike> {
        let gil = Python::acquire_gil();
        let py = gil.python();

        // is a path
        if let Ok(string_ref) = path_or_file_like.cast_as::<PyString>(py) {
            return Ok(FileOrFileLike::File(
                string_ref.to_string_lossy().to_string(),
            ));
        }

        // is a file-like
        match PyFileLikeObject::new(path_or_file_like) {
            Ok(f) => Ok(FileOrFileLike::FileLike(f)),
            Err(e) => Err(e),
        }
    }
}

#[pyfunction]
fn accepts_path_or_file_like(path_or_file_like: PyObject) -> PyResult<usize> {
    match FileOrFileLike::from_pyobject(path_or_file_like) {
        Ok(f) => match f {
            FileOrFileLike::File(s) => {
                println!("It's a file! - path {}", s);
                // Open file with std::fs::File..
                Ok(0)
            },
            FileOrFileLike::FileLike(mut f) => {
                println!("Its a file-like object");
                // Read something from it!
                
                let mut buffer = vec![0; 4096]; 
                Ok(f.read(&mut buffer)?)
            }
        },
        Err(e) => Err(e)
    }
}

#[pymodule]
fn example_module(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_wrapped(wrap_pyfunction!(accepts_path_or_file_like))?;

    Ok(())
}

# fn main() {}
```

and use from python:

```python
from example_module import accepts_path_or_file_like

def main():
    # works
    accepts_path_or_file_like("./some_file.txt")
    
    # also works
    f = open('./some_file.txt')
    accepts_path_or_file_like(f)
```
