use pyo3::intern;
use pyo3::{exceptions::PyTypeError, prelude::*};
use std::borrow::Cow;

use pyo3::types::{PyBytes, PyString};

use std::io;
use std::io::{Read, Seek, SeekFrom, Write};
#[cfg(unix)]
use std::os::fd::{AsRawFd, RawFd};

#[derive(Debug)]
pub struct PyFileLikeObject {
    // We use PyObject instead of Bound<PyAny> because Bound<PyAny> is a GIL-bound type.
    // We want to avoid holding the GIL when creating the struct.
    // The GIL will be re-taken when the methods are called.
    inner: PyObject,
    is_text_io: bool,
}

impl Clone for PyFileLikeObject {
    fn clone(&self) -> Self {
        Python::with_gil(|py| PyFileLikeObject {
            inner: self.inner.clone_ref(py),
            is_text_io: self.is_text_io,
        })
    }
}

/// Wraps a `PyObject`, and implements read, seek, and write for it.
impl PyFileLikeObject {
    /// Creates an instance of a `PyFileLikeObject` from a `PyObject`.
    /// To assert the object has the required methods methods,
    /// instantiate it with `PyFileLikeObject::require`
    pub fn new(object: PyObject) -> PyResult<Self> {
        Python::with_gil(|py| Self::py_new(object.into_bound(py)))
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
            Self::py_with_requirements(object.into_bound(py), read, write, seek, fileno)
        })
    }
}

impl PyFileLikeObject {
    pub fn py_new(obj: Bound<PyAny>) -> PyResult<Self> {
        let text_io = consts::text_io_base(obj.py())?;
        let is_text_io = obj.is_instance(text_io)?;

        Ok(PyFileLikeObject {
            inner: obj.unbind(),
            is_text_io,
        })
    }

    pub fn py_with_requirements(
        obj: Bound<PyAny>,
        read: bool,
        write: bool,
        seek: bool,
        fileno: bool,
    ) -> PyResult<Self> {
        if read && !obj.hasattr(consts::read(obj.py()))? {
            return Err(PyTypeError::new_err(
                "Object does not have a .read() method.",
            ));
        }

        if seek && !obj.hasattr(consts::seek(obj.py()))? {
            return Err(PyTypeError::new_err(
                "Object does not have a .seek() method.",
            ));
        }

        if write && !obj.hasattr(consts::write(obj.py()))? {
            return Err(PyTypeError::new_err(
                "Object does not have a .write() method.",
            ));
        }

        if fileno && !obj.hasattr(consts::fileno(obj.py()))? {
            return Err(PyTypeError::new_err(
                "Object does not have a .fileno() method.",
            ));
        }

        PyFileLikeObject::py_new(obj)
    }

    pub fn py_read(&self, py: Python<'_>, mut buf: &mut [u8]) -> io::Result<usize> {
        let inner = self.inner.bind(py);
        if self.is_text_io {
            if buf.len() < 4 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "buffer size must be at least 4 bytes",
                ));
            }
            let res = inner.call_method1(consts::read(py), (buf.len() / 4,))?;
            let rust_string = res.extract::<Cow<str>>()?;
            let bytes = rust_string.as_bytes();
            buf.write_all(bytes)?;
            Ok(bytes.len())
        } else {
            let pybytes = inner.call_method1(consts::read(py), (buf.len(),))?;
            let bytes = pybytes.extract::<Cow<[u8]>>()?;
            buf.write_all(&bytes)?;
            Ok(bytes.len())
        }
    }

    pub fn py_write(&self, py: Python<'_>, buf: &[u8]) -> io::Result<usize> {
        let inner = self.inner.bind(py);
        let arg = if self.is_text_io {
            let s =
                std::str::from_utf8(buf).expect("Tried to write non-utf8 data to a TextIO object.");
            PyString::new(py, s).into_any()
        } else {
            PyBytes::new(py, buf).into_any()
        };

        let number_bytes_written = inner.call_method1(consts::write(py), (arg,))?;

        if number_bytes_written.is_none() {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "write() returned None, expected number of bytes written",
            ));
        }

        number_bytes_written.extract().map_err(io::Error::from)
    }

    pub fn py_flush(&self, py: Python<'_>) -> io::Result<()> {
        self.inner.call_method0(py, consts::flush(py))?;
        Ok(())
    }

    pub fn py_seek(&self, py: Python<'_>, pos: SeekFrom) -> io::Result<u64> {
        let inner = self.inner.bind(py);
        let (whence, offset) = match pos {
            SeekFrom::Start(offset) => (0, offset as i64),
            SeekFrom::End(offset) => (2, offset),
            SeekFrom::Current(offset) => (1, offset),
        };

        let res = inner.call_method1(consts::seek(py), (offset, whence))?;
        res.extract().map_err(io::Error::from)
    }

    #[cfg(unix)]
    pub fn py_as_raw_fd(&self, py: Python<'_>) -> RawFd {
        let inner = self.inner.bind(py);
        let fd = inner
            .call_method0(consts::fileno(py))
            .expect("Object does not have a fileno() method.");

        fd.extract().expect("File descriptor is not an integer.")
    }

    pub fn py_clone(&self, py: Python<'_>) -> PyFileLikeObject {
        PyFileLikeObject {
            inner: self.inner.clone_ref(py),
            is_text_io: self.is_text_io,
        }
    }

    /// Access the name of the underlying file, if one exists
    /// https://docs.python.org/3/library/io.html#io.FileIO.name
    pub fn py_name(&self, py: Python<'_>) -> Option<String> {
        let py_obj = self.inner.getattr(py, intern!(py, "name")).ok()?;
        py_obj.extract::<String>(py).ok()
    }
}

impl Read for PyFileLikeObject {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
        Python::with_gil(|py| self.py_read(py, buf))
    }
}

impl Read for &PyFileLikeObject {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
        Python::with_gil(|py| self.py_read(py, buf))
    }
}

impl Write for PyFileLikeObject {
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        Python::with_gil(|py| self.py_write(py, buf))
    }

    fn flush(&mut self) -> Result<(), io::Error> {
        Python::with_gil(|py| self.py_flush(py))
    }
}

impl Write for &PyFileLikeObject {
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        Python::with_gil(|py| self.py_write(py, buf))
    }

    fn flush(&mut self) -> Result<(), io::Error> {
        Python::with_gil(|py| self.py_flush(py))
    }
}

impl Seek for PyFileLikeObject {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, io::Error> {
        Python::with_gil(|py| self.py_seek(py, pos))
    }
}

impl Seek for &PyFileLikeObject {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, io::Error> {
        Python::with_gil(|py| self.py_seek(py, pos))
    }
}

#[cfg(unix)]
impl AsRawFd for PyFileLikeObject {
    fn as_raw_fd(&self) -> RawFd {
        Python::with_gil(|py| self.py_as_raw_fd(py))
    }
}

#[cfg(unix)]
impl AsRawFd for &PyFileLikeObject {
    fn as_raw_fd(&self) -> RawFd {
        Python::with_gil(|py| self.py_as_raw_fd(py))
    }
}

mod consts {
    use pyo3::prelude::*;
    use pyo3::sync::GILOnceCell;
    use pyo3::types::PyString;
    use pyo3::{intern, Bound, Py, PyResult, Python};

    pub fn fileno(py: Python) -> &Bound<PyString> {
        intern!(py, "fileno")
    }

    pub fn read(py: Python) -> &Bound<PyString> {
        intern!(py, "read")
    }

    pub fn write(py: Python<'_>) -> &Bound<PyString> {
        intern!(py, "write")
    }

    pub fn seek(py: Python<'_>) -> &Bound<PyString> {
        intern!(py, "seek")
    }

    pub fn flush(py: Python<'_>) -> &Bound<PyString> {
        intern!(py, "flush")
    }

    pub fn text_io_base(py: Python) -> PyResult<&Bound<PyAny>> {
        static INSTANCE: GILOnceCell<Py<PyAny>> = GILOnceCell::new();

        INSTANCE
            .get_or_try_init(py, || {
                let io = PyModule::import(py, "io")?;
                let cls = io.getattr("TextIOBase")?;
                Ok(cls.unbind())
            })
            .map(|x| x.bind(py))
    }
}
