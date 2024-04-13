use pyo3::{exceptions::PyTypeError, prelude::*};

use pyo3::types::{PyBytes, PyString};

use std::io;
use std::io::{Read, Seek, SeekFrom, Write};
#[cfg(not(target_os = "windows"))]
use std::os::fd::{AsRawFd, RawFd};

#[derive(Debug, Clone)]
pub struct PyFileLikeObject {
    inner: PyObject,
    is_text_io: bool,
}

/// Wraps a `PyObject`, and implements read, seek, and write for it.
impl PyFileLikeObject {
    /// Creates an instance of a `PyFileLikeObject` from a `PyObject`.
    /// To assert the object has the required methods methods,
    /// instantiate it with `PyFileLikeObject::require`
    pub fn new(object: PyObject) -> PyResult<Self> {
        Python::with_gil(|py| {
            let text_io = consts::text_io_base(py)?;

            let is_text_io = object.bind(py).is_instance(text_io)?;

            Ok(PyFileLikeObject {
                inner: object,
                is_text_io,
            })
        })
    }

    /// Same as `PyFileLikeObject::new`, but validates that the underlying
    /// python object has a `read`, `write`, and `seek` methods in respect to parameters.
    /// Will return a `TypeError` if object does not have `read`, `seek`, `write` and `fileno` methods.
    pub fn with_requirements(
        object: PyObject,
        read: bool,
        write: bool,
        seek: bool,
        fileno: bool,
    ) -> PyResult<Self> {
        Python::with_gil(|py| {
            if read && object.getattr(py, consts::read(py)).is_err() {
                return Err(PyErr::new::<PyTypeError, _>(
                    "Object does not have a .read() method.",
                ));
            }

            if seek && object.getattr(py, consts::seek(py)).is_err() {
                return Err(PyErr::new::<PyTypeError, _>(
                    "Object does not have a .seek() method.",
                ));
            }

            if write && object.getattr(py, consts::write(py)).is_err() {
                return Err(PyErr::new::<PyTypeError, _>(
                    "Object does not have a .write() method.",
                ));
            }

            if fileno && object.getattr(py, consts::fileno(py)).is_err() {
                return Err(PyErr::new::<PyTypeError, _>(
                    "Object does not have a .fileno() method.",
                ));
            }

            PyFileLikeObject::new(object)
        })
    }
}

impl Read for PyFileLikeObject {
    fn read(&mut self, mut buf: &mut [u8]) -> Result<usize, io::Error> {
        Python::with_gil(|py| {
            if self.is_text_io {
                if buf.len() < 4 {
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        "buffer size must be at least 4 bytes",
                    ));
                }
                let res =
                    self.inner
                        .call_method_bound(py, consts::read(py), (buf.len() / 4,), None)?;
                let pystring = res
                    .downcast_bound::<PyString>(py)
                    .expect("Expecting to be able to downcast into str from read result.");

                let rust_string = pystring.extract::<String>().unwrap();
                let bytes = rust_string.as_bytes();
                buf.write_all(bytes)?;
                Ok(bytes.len())
            } else {
                let res = self
                    .inner
                    .call_method_bound(py, consts::read(py), (buf.len(),), None)?;
                let pybytes = res
                    .downcast_bound(py)
                    .expect("Expecting to be able to downcast into bytes from read result.");
                let bytes = pybytes.extract().unwrap();
                buf.write_all(bytes)?;
                Ok(bytes.len())
            }
        })
    }
}

impl Write for PyFileLikeObject {
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        Python::with_gil(|py| {
            let arg = if self.is_text_io {
                let s = std::str::from_utf8(buf)
                    .expect("Tried to write non-utf8 data to a TextIO object.");
                PyString::new_bound(py, s).to_object(py)
            } else {
                PyBytes::new_bound(py, buf).to_object(py)
            };

            let number_bytes_written =
                self.inner
                    .call_method_bound(py, consts::write(py), (arg,), None)?;

            if number_bytes_written.is_none(py) {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "write() returned None, expected number of bytes written",
                ));
            }

            number_bytes_written.extract(py).map_err(io::Error::from)
        })
    }

    fn flush(&mut self) -> Result<(), io::Error> {
        Python::with_gil(|py| {
            self.inner
                .call_method_bound(py, consts::flush(py), (), None)?;

            Ok(())
        })
    }
}

impl Seek for PyFileLikeObject {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, io::Error> {
        Python::with_gil(|py| {
            let (whence, offset) = match pos {
                SeekFrom::Start(i) => (0, i as i64),
                SeekFrom::Current(i) => (1, i),
                SeekFrom::End(i) => (2, i),
            };

            let new_position =
                self.inner
                    .call_method_bound(py, consts::seek(py), (offset, whence), None)?;

            new_position.extract(py).map_err(io::Error::from)
        })
    }
}

#[cfg(not(target_os = "windows"))]
impl AsRawFd for PyFileLikeObject {
    fn as_raw_fd(&self) -> RawFd {
        Python::with_gil(|py| {
            let fileno = self
                .inner
                .getattr(py, consts::fileno(py))
                .expect("Object does not have a fileno() method.");

            let fd = fileno
                .call_bound(py, (), None)
                .expect("fileno() method did not return a file descriptor.");

            fd.extract(py).expect("File descriptor is not an integer.")
        })
    }
}

mod consts {
    use pyo3::prelude::*;
    use pyo3::sync::GILOnceCell;
    use pyo3::types::PyString;
    use pyo3::{intern, Bound, Py, PyResult, Python};

    pub fn fileno<'py>(py: Python<'py>) -> &'py Bound<PyString> {
        intern!(py, "fileno")
    }

    pub fn read<'py>(py: Python<'py>) -> &'py Bound<PyString> {
        intern!(py, "read")
    }

    pub fn write<'py>(py: Python<'_>) -> &'py Bound<PyString> {
        intern!(py, "write")
    }

    pub fn seek<'py>(py: Python<'_>) -> &'py Bound<PyString> {
        intern!(py, "seek")
    }

    pub fn flush<'py>(py: Python<'_>) -> &'py Bound<PyString> {
        intern!(py, "flush")
    }

    pub fn text_io_base<'py>(py: Python<'py>) -> PyResult<&'py Bound<PyAny>> {
        static INSTANCE: GILOnceCell<Py<PyAny>> = GILOnceCell::new();

        INSTANCE
            .get_or_try_init(py, || {
                let io = PyModule::import_bound(py, "io")?;
                let cls = io.getattr("TextIOBase")?;
                Ok(cls.unbind())
            })
            .map(|x| x.bind(py))
    }
}
