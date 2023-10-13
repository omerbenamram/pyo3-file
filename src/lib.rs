use pyo3::{exceptions::PyTypeError, prelude::*};

use pyo3::types::{PyBytes, PyString};

use pyo3::once_cell::GILOnceCell;
use std::io;
use std::io::{Read, Seek, SeekFrom, Write};

#[derive(Debug, Clone)]
pub struct PyFileLikeObject {
    inner: PyObject,
    is_text_io: bool,
}

impl<'a> FromPyObject<'a> for PyFileLikeObject {
    fn extract(ob: &'a PyAny) -> PyResult<Self> {
        Self::new(ob)
    }
}

impl ToPyObject for PyFileLikeObject {
    fn to_object(&self, py: Python<'_>) -> PyObject {
        self.inner.clone_ref(py)
    }
}

impl IntoPy<PyObject> for PyFileLikeObject {
    fn into_py(self, _py: Python<'_>) -> PyObject {
        self.inner
    }
}

fn text_io_base_type(py: Python<'_>) -> PyResult<&'_ PyAny> {
    static TEXT_IO_BASE_TYPE: GILOnceCell<PyObject> = GILOnceCell::new();

    let obj: &PyObject = TEXT_IO_BASE_TYPE.get_or_try_init(py, || -> PyResult<PyObject> {
        let io = PyModule::import(py, "io")?;
        let base_type = io.getattr("TextIOBase")?;
        let base_type: PyObject = base_type.into();
        Ok(base_type)
    })?;

    Ok(obj.as_ref(py))
}

/// Wraps a `PyObject`, and implements read, seek, and write for it.
impl PyFileLikeObject {
    /// Creates an instance of a `PyFileLikeObject` from a `PyObject`.
    /// To assert the object has the required methods methods,
    /// instantiate it with `PyFileLikeObject::require`
    pub fn new(object: &PyAny) -> PyResult<Self> {
        let py = object.py();
        let text_io_base_type = text_io_base_type(py)?;
        let is_text_io = object.is_instance(text_io_base_type)?;

        Ok(PyFileLikeObject {
            inner: object.into(),
            is_text_io,
        })
    }

    /// Function to create and validate a `PyFileLikeObject`.
    ///
    /// This uses const generics, in order to be usable for `pyo3(from_py_with = "...")`
    ///
    /// ```rust
    /// use std::io::Read;
    /// use pyo3::prelude::*;
    /// use pyo3_file::PyFileLikeObject;
    ///
    /// #[pyfunction]
    /// fn my_method(
    ///     #[pyo3(from_py_with = "PyFileLikeObject::with_rw_seek::<false, true, false>")]
    ///     mut file: PyFileLikeObject
    /// ) -> PyResult<()> {
    ///    let mut buf = [0; 1024];
    ///    let n = file.read(&mut buf)?;
    ///    println!("Read bytes: {:?}", &buf[..n]);
    /// Ok(())
    /// }
    /// ```
    pub fn with_rw_seek<const READ: bool, const WRITE: bool, const SEEK: bool>(
        object: &PyAny,
    ) -> PyResult<Self> {
        Self::with_requirements(object, READ, WRITE, SEEK)
    }

    /// Same as `PyFileLikeObject::new`, but validates that the underlying
    /// python object has a `read`, `write`, and `seek` methods in respect to parameters.
    /// Will return a `TypeError` if object does not have `read`, `seek`, and `write` methods.
    pub fn with_requirements(
        object: &PyAny,
        read: bool,
        write: bool,
        seek: bool,
    ) -> PyResult<Self> {
        let py = object.py();
        let this = Self::new(object)?;
        if read {
            this.check_readable(py)?;
        }

        if seek {
            this.check_seekable(py)?;
        }

        if write {
            this.check_writable(py)?;
        }

        Ok(this)
    }

    /// Makes a clone of self.
    ///
    /// This creates another pointer to the same object, increasing its reference count.
    ///
    /// You should prefer using this method over [`Clone`] if you happen to be holding the GIL already.
    pub fn clone_ref(&self, py: Python<'_>) -> Self {
        Self {
            inner: self.inner.clone_ref(py),
            is_text_io: self.is_text_io,
        }
    }

    /// Checks if the underlying python object has a `read` method, and that it is readable.
    pub fn check_readable(&self, py: Python<'_>) -> PyResult<()> {
        let object = self.inner.as_ref(py);
        if object.getattr(pyo3::intern!(py, "read")).is_err() {
            return Err(PyTypeError::new_err("object does not have a read() method"));
        }
        let readable_res = object
            .call_method0(pyo3::intern!(py, "readable"))
            .and_then(|res| res.is_true());
        if matches!(readable_res, Ok(false)) {
            return Err(PyTypeError::new_err("object is not readable"));
        }
        Ok(())
    }

    /// Checks if the underlying python object has a `write` method, and that it is writable.
    pub fn check_writable(&self, py: Python<'_>) -> PyResult<()> {
        let object = self.inner.as_ref(py);
        if object.getattr(pyo3::intern!(py, "write")).is_err() {
            return Err(PyTypeError::new_err(
                "object does not have a write() method",
            ));
        }
        let writable_res = object
            .call_method0(pyo3::intern!(py, "writable"))
            .and_then(|res| res.is_true());
        if matches!(writable_res, Ok(false)) {
            return Err(PyTypeError::new_err("object is not writable"));
        }
        Ok(())
    }

    /// Checks if the underlying python object has a `seek` method, and that it is seekable.
    pub fn check_seekable(&self, py: Python<'_>) -> PyResult<()> {
        let object = self.inner.as_ref(py);
        if object.getattr(pyo3::intern!(py, "seek")).is_err() {
            return Err(PyTypeError::new_err("object does not have a seek() method"));
        }
        let seekable_res = object
            .call_method0(pyo3::intern!(py, "seekable"))
            .and_then(|res| res.is_true());
        if matches!(seekable_res, Ok(false)) {
            return Err(PyTypeError::new_err("object is not seekable"));
        }
        Ok(())
    }

    /// Try to get the underlying fileno of the python object.
    pub fn fileno(&self, py: Python<'_>) -> PyResult<i64> {
        let fileno_result = self.inner.call_method0(py, pyo3::intern!(py, "fileno"))?;
        fileno_result.extract::<i64>(py)
    }
}

/// Create a rust io::Error from a python exception
fn pyerr_to_io_err(py: Python<'_>, e: PyErr) -> io::Error {
    let e_as_object = e.value(py);
    let kind = match e_as_object
        .getattr(pyo3::intern!(py, "errno"))
        .and_then(PyAny::extract)
    {
        Ok(errno) => io::Error::from_raw_os_error(errno).kind(),
        Err(..) => io::ErrorKind::Other,
    };
    io::Error::new(kind, e)
}

impl Read for &PyFileLikeObject {
    fn read(&mut self, mut buf: &mut [u8]) -> Result<usize, io::Error> {
        Python::with_gil(|py| {
            // Pre-declare, so bytes can reference it
            let res: PyObject;
            let bytes = if self.is_text_io {
                if buf.len() < 4 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "buffer size must be at least 4 bytes",
                    ));
                }
                res = self
                    .inner
                    .call_method1(py, "read", (buf.len() / 4,))
                    .map_err(|e| pyerr_to_io_err(py, e))?;
                let string: &str = res.extract(py).map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::Other,
                        "read() on text-based objects should return a unicode string",
                    )
                })?;
                string.as_bytes()
            } else {
                res = self
                    .inner
                    .call_method1(py, "read", (buf.len(),))
                    .map_err(|e| pyerr_to_io_err(py, e))?;
                let bytes: &[u8] = res.extract(py).map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::Other,
                        "read() on binary objects should return a bytes object",
                    )
                })?;
                bytes
            };
            buf.write_all(bytes)?;
            Ok(bytes.len())
        })
    }
}

impl Read for PyFileLikeObject {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
        <&PyFileLikeObject>::read(&mut &*self, buf)
    }
}

impl Write for &PyFileLikeObject {
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        Python::with_gil(|py| {
            let arg = if self.is_text_io {
                let s = std::str::from_utf8(buf).map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "Can only write utf8 data to a TextIO object",
                    )
                })?;
                PyString::new(py, s).to_object(py)
            } else {
                PyBytes::new(py, buf).to_object(py)
            };

            let number_bytes_written = self
                .inner
                .call_method1(py, "write", (arg,))
                .map_err(|e| pyerr_to_io_err(py, e))?;

            if number_bytes_written.is_none(py) {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "write() returned None, expected number of bytes written",
                ));
            }

            number_bytes_written
                .extract(py)
                .map_err(|e| pyerr_to_io_err(py, e))
        })
    }

    fn flush(&mut self) -> Result<(), io::Error> {
        Python::with_gil(|py| {
            self.inner
                .call_method(py, "flush", (), None)
                .map_err(|e| pyerr_to_io_err(py, e))?;

            Ok(())
        })
    }
}

impl Write for PyFileLikeObject {
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        <&PyFileLikeObject>::write(&mut &*self, buf)
    }

    fn flush(&mut self) -> Result<(), io::Error> {
        <&PyFileLikeObject>::flush(&mut &*self)
    }
}

impl Seek for &PyFileLikeObject {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, io::Error> {
        Python::with_gil(|py| {
            let (whence, offset) = match pos {
                SeekFrom::Start(i) => (0, i as i64),
                SeekFrom::Current(i) => (1, i),
                SeekFrom::End(i) => (2, i),
            };

            let new_position = self
                .inner
                .call_method1(py, "seek", (offset, whence))
                .map_err(|e| pyerr_to_io_err(py, e))?;

            new_position.extract(py).map_err(|e| pyerr_to_io_err(py, e))
        })
    }
}

impl Seek for PyFileLikeObject {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, io::Error> {
        <&PyFileLikeObject>::seek(&mut &*self, pos)
    }
}
