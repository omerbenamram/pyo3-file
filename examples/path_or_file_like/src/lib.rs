use pyo3::types::PyString;
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

impl FileOrFileLike {
    pub fn from_pyobject(path_or_file_like: PyObject) -> PyResult<FileOrFileLike> {
        Python::with_gil(|py| {
            // is a path
            if let Ok(string_ref) = path_or_file_like.downcast_bound::<PyString>(py) {
                return Ok(FileOrFileLike::File(
                    string_ref.to_string_lossy().to_string(),
                ));
            }

            // is a file-like
            match PyFileLikeObject::with_requirements(path_or_file_like, true, false, true, false) {
                Ok(f) => Ok(FileOrFileLike::FileLike(f)),
                Err(e) => Err(e),
            }
        })
    }
}

#[pyfunction]
/// Opens a file or file-like, and reads it to string.
fn accepts_path_or_file_like_read(path_or_file_like: PyObject) -> PyResult<String> {
    match FileOrFileLike::from_pyobject(path_or_file_like) {
        Ok(f) => match f {
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
        },
        Err(e) => Err(e),
    }
}

#[pyfunction]
/// Opens a file or file-like, and write a string to it.
fn accepts_file_like_write(file_like: PyObject) -> PyResult<()> {
    // is a file-like
    match PyFileLikeObject::with_requirements(file_like, false, true, false, false) {
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
