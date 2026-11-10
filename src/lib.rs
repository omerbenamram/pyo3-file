#![doc = include_str!("../README.md")]

use pyo3::intern;
use pyo3::{exceptions::PyTypeError, prelude::*};
use std::borrow::Cow;

use pyo3::types::{PyBytes, PyString};
#[cfg(feature = "experimental-inspect")]
use pyo3::{type_hint_identifier, type_hint_subscript, type_hint_union};

use std::io;
use std::io::{Read, Seek, SeekFrom, Write};
#[cfg(unix)]
use std::os::fd::{AsRawFd, RawFd};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// A wrapper around a Python object that implements the file-like interface.
#[derive(Debug)]
pub struct PyFileLikeObject {
    // We use PyObject instead of Bound<PyAny> because Bound<PyAny> is a GIL-bound type.
    // We want to avoid holding the GIL when creating the struct.
    // The GIL will be re-taken when the methods are called.
    inner: Py<PyAny>,
    is_text_io: bool,
    /// The bytes of a text stream that are between the two sides. Clones share them, as they
    /// share the stream.
    text: Arc<Mutex<TextBuffers>>,
}

/// A text stream takes and gives whole characters, and `Read` and `Write` work in bytes, which
/// can stop inside a character's UTF-8 encoding. The bytes in between belong to this wrapper
/// and its clones: another wrapper of the same Python object, or Python code using the object,
/// doesn't see them, and threads that read or write one text stream at once mix them up, as
/// they would the stream.
#[derive(Debug, Default)]
struct TextBuffers {
    /// Bytes of a character read from the stream that the caller's buffer had no room for: at
    /// most four.
    read: Vec<u8>,
    /// The first bytes of a character whose other bytes the caller has yet to write: at most
    /// three.
    written: Vec<u8>,
}

impl Clone for PyFileLikeObject {
    fn clone(&self) -> Self {
        Python::attach(|py| self.py_clone(py))
    }
}

/// Wraps a `PyObject`, and implements read, seek, and write for it.
impl PyFileLikeObject {
    /// Creates an instance of a `PyFileLikeObject` from a `PyObject`.
    ///
    /// To assert the object has the required methods methods,
    /// instantiate it with [`with_requirements`][Self::with_requirements].
    ///
    /// Prefer using [`py_new`][Self::py_new] if you already have a `Bound<PyAny>` object, as this
    /// method re-acquires the GIL internally.
    pub fn new(object: Py<PyAny>) -> PyResult<Self> {
        Python::attach(|py| Self::py_new(object.into_bound(py)))
    }

    /// Same as `PyFileLikeObject::new`, but validates that the underlying
    /// python object has a `read`, `write`, and `seek` methods in respect to parameters.
    /// Will return a `TypeError` if object does not have `read`, `seek`, `write` and `fileno` methods.
    ///
    /// Prefer using [`py_with_requirements`][Self::py_with_requirements] if you already have a
    /// `Bound<PyAny>` object, as this method re-acquires the GIL internally.
    pub fn with_requirements(
        object: Py<PyAny>,
        read: bool,
        write: bool,
        seek: bool,
        fileno: bool,
    ) -> PyResult<Self> {
        Python::attach(|py| {
            Self::py_with_requirements(object.into_bound(py), read, write, seek, fileno)
        })
    }

    /// Creates an instance of a `PyFileLikeObject` from a `PyObject`.
    ///
    /// Prefer using this instead of [`new`][Self::new] if you already have a `Bound<PyAny>`
    /// object, as this method does not acquire the GIL internally.
    pub fn py_new(obj: Bound<PyAny>) -> PyResult<Self> {
        let text_io = consts::text_io_base(obj.py())?;
        let is_text_io = obj.is_instance(text_io)?;

        Ok(PyFileLikeObject {
            inner: obj.unbind(),
            is_text_io,
            text: Arc::default(),
        })
    }

    /// Creates an instance of a `PyFileLikeObject` from a `PyObject`.
    ///
    /// Prefer using this instead of [`with_requirements`][Self::with_requirements] if you already
    /// have a `Bound<PyAny>` object, as this method does not acquire the GIL internally.
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

    /// Reads into `buf` with the object's `read()`.
    ///
    /// A text stream (an `io.TextIOBase`) is read as the UTF-8 encoding of its characters. A
    /// character that `buf` has no room for is kept for the next read, so `buf` can be of any
    /// size.
    pub fn py_read(&self, py: Python<'_>, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let inner = self.inner.bind(py);
        if self.is_text_io {
            // The buffers aren't locked while Python runs: its code can let another thread in.
            if self.text_buffers().read.is_empty() {
                // A character is at most four bytes.
                let chars = (buf.len() / 4).max(1);
                let res = inner.call_method1(consts::read(py), (chars,))?;
                let rust_string = res.extract::<Cow<str>>()?;
                let bytes = rust_string.as_bytes();
                if let Some(dest) = buf.get_mut(..bytes.len()) {
                    dest.copy_from_slice(bytes);
                    return Ok(bytes.len());
                }
                // A buffer of under four bytes, and a character longer than it.
                self.text_buffers().read.extend_from_slice(bytes);
            }
            let mut text = self.text_buffers();
            let len = text.read.len().min(buf.len());
            buf[..len].copy_from_slice(&text.read[..len]);
            text.read.drain(..len);
            Ok(len)
        } else {
            let pybytes = inner.call_method1(consts::read(py), (buf.len(),))?;
            let bytes = pybytes
                .extract::<Cow<[u8]>>()
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err.to_string()))?;
            let Some(dest) = buf.get_mut(..bytes.len()) else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("read({}) returned {} bytes", buf.len(), bytes.len()),
                ));
            };
            dest.copy_from_slice(&bytes);
            Ok(bytes.len())
        }
    }

    /// Writes `buf` with the object's `write()`, and returns how many of its bytes were taken.
    ///
    /// A text stream (an `io.TextIOBase`) is written the characters that `buf` encodes as UTF-8.
    /// `buf` may end inside a character, as a buffered writer's does: those bytes are kept
    /// until the rest of the character is written. Bytes that aren't UTF-8 are an
    /// [`io::ErrorKind::InvalidData`] error.
    pub fn py_write(&self, py: Python<'_>, buf: &[u8]) -> io::Result<usize> {
        let inner = self.inner.bind(py);
        if self.is_text_io {
            // The bytes kept from the write before come first. The buffers aren't locked while
            // Python runs: its code can let another thread in.
            let before = self.text_buffers().written.clone();
            let kept = before.len();
            let bytes: Cow<[u8]> = if kept == 0 {
                Cow::Borrowed(buf)
            } else {
                Cow::Owned([&before[..], buf].concat())
            };
            let (valid, invalid) = match std::str::from_utf8(&bytes) {
                Ok(s) => (s, false),
                Err(e) => (
                    // Its first `valid_up_to` bytes are UTF-8.
                    std::str::from_utf8(&bytes[..e.valid_up_to()]).unwrap_or_default(),
                    e.error_len().is_some(),
                ),
            };
            if valid.is_empty() && invalid {
                // The bytes kept are part of what is refused.
                self.text_buffers().written.clear();
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "tried to write bytes that are not UTF-8 to a text stream",
                ));
            }
            if !valid.is_empty() {
                // If it fails, the bytes kept stay kept, and the caller may write `buf` again.
                self.write_text(inner, valid)?;
            }
            // Invalid bytes are left to the next write, which fails on them. Otherwise what
            // follows the characters written is the start of a character, to keep.
            let taken = if invalid { valid.len() } else { bytes.len() };
            self.text_buffers().written = bytes[valid.len()..taken].to_vec();
            return Ok(taken - kept);
        }

        let number_bytes_written =
            inner.call_method1(consts::write(py), (PyBytes::new(py, buf),))?;

        if number_bytes_written.is_none() {
            return Err(io::Error::other(
                "write() returned None, expected number of bytes written",
            ));
        }

        let written: usize = number_bytes_written.extract().map_err(io::Error::from)?;
        if written > buf.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("write() of {} bytes returned {written}", buf.len()),
            ));
        }
        Ok(written)
    }

    /// Writes `text` to a text stream, which takes all of a string or raises.
    fn write_text(&self, inner: &Bound<'_, PyAny>, text: &str) -> io::Result<()> {
        let py = inner.py();
        let written = inner.call_method1(consts::write(py), (PyString::new(py, text),))?;
        if written.is_none() {
            return Err(io::Error::other(
                "write() returned None, expected number of characters written",
            ));
        }
        let written: usize = written.extract().map_err(io::Error::from)?;
        let chars = text.chars().count();
        if written != chars {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("write() of {chars} characters returned {written}"),
            ));
        }
        Ok(())
    }

    fn text_buffers(&self) -> MutexGuard<'_, TextBuffers> {
        // A panic while it was held left the buffers usable.
        self.text.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Calls the object's `flush()`. For a text stream, bytes written last that stop inside a
    /// character are an [`io::ErrorKind::InvalidData`] error: they can't be flushed, and are
    /// still written if the rest of the character follows.
    pub fn py_flush(&self, py: Python<'_>) -> io::Result<()> {
        self.inner.call_method0(py, consts::flush(py))?;
        if self.is_text_io && !self.text_buffers().written.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "the bytes written to a text stream end inside a character",
            ));
        }
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
        // Bytes read ahead of the old position, or written short of a character at it, don't
        // belong to the new one. Asking for the position moves nothing.
        if self.is_text_io && pos != SeekFrom::Current(0) {
            *self.text_buffers() = TextBuffers::default();
        }
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
            text: Arc::clone(&self.text),
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
        Python::attach(|py| self.py_read(py, buf))
    }
}

impl Read for &PyFileLikeObject {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
        Python::attach(|py| self.py_read(py, buf))
    }
}

impl Write for PyFileLikeObject {
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        Python::attach(|py| self.py_write(py, buf))
    }

    fn flush(&mut self) -> Result<(), io::Error> {
        Python::attach(|py| self.py_flush(py))
    }
}

impl Write for &PyFileLikeObject {
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        Python::attach(|py| self.py_write(py, buf))
    }

    fn flush(&mut self) -> Result<(), io::Error> {
        Python::attach(|py| self.py_flush(py))
    }
}

impl Seek for PyFileLikeObject {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, io::Error> {
        Python::attach(|py| self.py_seek(py, pos))
    }
}

impl Seek for &PyFileLikeObject {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, io::Error> {
        Python::attach(|py| self.py_seek(py, pos))
    }
}

#[cfg(unix)]
impl AsRawFd for PyFileLikeObject {
    fn as_raw_fd(&self) -> RawFd {
        Python::attach(|py| self.py_as_raw_fd(py))
    }
}

#[cfg(unix)]
impl AsRawFd for &PyFileLikeObject {
    fn as_raw_fd(&self) -> RawFd {
        Python::attach(|py| self.py_as_raw_fd(py))
    }
}

impl<'py> FromPyObject<'_, 'py> for PyFileLikeObject {
    type Error = PyErr;

    /// The Python type a `PyFileLikeObject` argument is described by in generated type stubs.
    ///
    /// The hint is structural rather than `typing.IO[Any]` on purpose. `typing.IO` is a plain
    /// class, not a `Protocol`, so type checkers only accept nominal subclasses of it. That
    /// rejects, at check time, objects this crate handles perfectly at runtime: `gzip.GzipFile`,
    /// any custom subclass of `io.RawIOBase`/`io.BufferedIOBase`, and any plain object that just
    /// defines `.read()` — which is the whole point of the crate. `_typeshed.SupportsRead` and
    /// `_typeshed.SupportsWrite` are protocols, so they accept all of those.
    ///
    /// The parameter is `Any`, not `bytes`: [`py_read`][PyFileLikeObject::py_read] and
    /// [`py_write`][PyFileLikeObject::py_write] branch on whether the object is an
    /// `io.TextIOBase`, exchanging `str` with text streams and `bytes` with binary ones, so both
    /// `SupportsRead[str]` and `SupportsRead[bytes]` are legitimate inputs.
    #[cfg(feature = "experimental-inspect")]
    const INPUT_TYPE: pyo3::inspect::PyStaticExpr = type_hint_union!(
        type_hint_subscript!(
            type_hint_identifier!("_typeshed", "SupportsRead"),
            type_hint_identifier!("typing", "Any")
        ),
        type_hint_subscript!(
            type_hint_identifier!("_typeshed", "SupportsWrite"),
            type_hint_identifier!("typing", "Any")
        )
    );

    fn extract(obj: Borrowed<'_, 'py, PyAny>) -> Result<Self, Self::Error> {
        Self::py_new(obj.as_any().clone())
    }
}

#[cfg(all(test, feature = "experimental-inspect"))]
mod inspect_tests {
    use super::*;

    #[test]
    fn input_type_renders_as_a_structural_union() {
        assert_eq!(
            <PyFileLikeObject as FromPyObject>::INPUT_TYPE.to_string(),
            "_typeshed.SupportsRead[typing.Any] | _typeshed.SupportsWrite[typing.Any]"
        );
    }
}

mod consts {
    use pyo3::prelude::*;
    use pyo3::sync::PyOnceLock;
    use pyo3::types::PyString;
    use pyo3::{intern, Bound, Py, PyResult, Python};

    pub fn fileno(py: Python<'_>) -> &Bound<'_, PyString> {
        intern!(py, "fileno")
    }

    pub fn read(py: Python<'_>) -> &Bound<'_, PyString> {
        intern!(py, "read")
    }

    pub fn write(py: Python<'_>) -> &Bound<'_, PyString> {
        intern!(py, "write")
    }

    pub fn seek(py: Python<'_>) -> &Bound<'_, PyString> {
        intern!(py, "seek")
    }

    pub fn flush(py: Python<'_>) -> &Bound<'_, PyString> {
        intern!(py, "flush")
    }

    pub fn text_io_base(py: Python<'_>) -> PyResult<&Bound<'_, PyAny>> {
        static INSTANCE: PyOnceLock<Py<PyAny>> = PyOnceLock::new();

        INSTANCE
            .get_or_try_init(py, || {
                let io = PyModule::import(py, "io")?;
                let cls = io.getattr("TextIOBase")?;
                Ok(cls.unbind())
            })
            .map(|x| x.bind(py))
    }
}
