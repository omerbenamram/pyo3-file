use pyo3_file::PyFileLikeObject;

use pyo3::prelude::*;
use pyo3::wrap_pyfunction;
use std::fs::File;
use std::io::{Read, Write};

/// Represents either a path `File` or a file-like object `FileLike`
#[derive(Debug)]
enum FileOrFileLike {
    File(String),
    FileLike(PyFileLikeObject),
}

impl<'py> FromPyObject<'py> for FileOrFileLike {
    fn extract_bound(ob: &Bound<'py, PyAny>) -> PyResult<Self> {
        // is a path
        if let Ok(string) = ob.extract::<String>() {
            return Ok(FileOrFileLike::File(string));
        }

        // is a file-like
        let f = PyFileLikeObject::py_with_requirements(ob.clone(), true, false, true, false)?;
        Ok(FileOrFileLike::FileLike(f))
    }
}

#[pyfunction]
/// Opens a file or file-like, and reads it to string.
fn accepts_path_or_file_like_read(path_or_file_like: FileOrFileLike) -> PyResult<String> {
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

#[pyfunction]
/// Opens a file or file-like, and write a string to it.
fn accepts_file_like_write(file_like: Bound<PyAny>) -> PyResult<()> {
    // is a file-like
    match PyFileLikeObject::py_with_requirements(file_like, false, true, false, false) {
        Ok(mut f) => {
            println!("Its a file-like object");
            f.write_all(b"Hello, world!")?;
            Ok(())
        }
        Err(e) => Err(e),
    }
}

#[pyfunction]
/// Access the name of a file-like object
fn name_of_file_like(py: Python, file_like: PyObject) -> PyResult<Option<String>> {
    // is a file-like
    match PyFileLikeObject::with_requirements(file_like, false, true, false, false) {
        Ok(f) => Ok(f.py_name(py)),
        Err(e) => Err(e),
    }
}

#[pymodule]
fn path_or_file_like(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_wrapped(wrap_pyfunction!(accepts_path_or_file_like_read))?;
    m.add_wrapped(wrap_pyfunction!(accepts_file_like_write))?;
    m.add_wrapped(wrap_pyfunction!(name_of_file_like))?;

    Ok(())
}
