use pyo3_file::PyFileLikeObject;

use pyo3::prelude::*;
use pyo3::wrap_pyfunction;
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;

/// Represents either a path `File` or a file-like object `FileLike`
#[derive(Debug, FromPyObject)]
enum FileOrFileLike {
    #[pyo3(annotation = "path")]
    FileName(PathBuf),
    #[pyo3(annotation = "file-like")]
    FileLike(
        #[pyo3(from_py_with = "PyFileLikeObject::with_rw_seek::<true, false, true>")]
        PyFileLikeObject,
    ),
}

#[pyfunction]
/// Opens a file or file-like, and reads it to string.
fn accepts_path_or_file_like_read(f: FileOrFileLike) -> PyResult<String> {
    match f {
        FileOrFileLike::FileName(s) => {
            println!("It's a file! - path {}", s.display());
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
fn accepts_file_like_write(
    #[pyo3(from_py_with = "PyFileLikeObject::with_rw_seek::<false, true, false>")]
    mut file_like: PyFileLikeObject,
) -> PyResult<()> {
    println!("Its a file-like object");
    file_like.write_all(b"Hello, world!")?;
    Ok(())
}

#[pymodule]
fn path_or_file_like(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_wrapped(wrap_pyfunction!(accepts_path_or_file_like_read))?;
    m.add_wrapped(wrap_pyfunction!(accepts_file_like_write))?;

    Ok(())
}
