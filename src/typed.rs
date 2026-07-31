//! A file-like wrapper that carries its method requirements in its type.
//!
//! [`PyFileLikeObject`] accepts any object and leaves it to the extracting function to say, via
//! [`py_with_requirements`][PyFileLikeObject::py_with_requirements], which of `read`, `write`,
//! `seek` and `fileno` it actually needs. That choice is invisible to everything else: the Rust
//! type is the same either way, and so is the annotation the type stubs get.
//!
//! [`PyFileLike`] moves those four flags into const generic parameters. One declaration then drives
//! three things that previously had to be kept in sync by hand:
//!
//! * the runtime check performed during extraction,
//! * which of [`Read`], [`Write`], [`Seek`] and [`AsRawFd`] the type implements, so that requiring
//!   too little is a compile error rather than a runtime `TypeError`,
//! * the annotation written into generated type stubs.

use std::io;
use std::io::{Read, Seek, SeekFrom, Write};
#[cfg(unix)]
use std::os::fd::{AsRawFd, RawFd};

use pyo3::prelude::*;

use crate::PyFileLikeObject;

/// A [`PyFileLikeObject`] whose required methods are part of its type.
///
/// The parameters are, in order, whether `read`, `write`, `seek` and `fileno` are required.
/// Extraction fails with a `TypeError` if the object is missing any of them, exactly as
/// [`PyFileLikeObject::py_with_requirements`] would with the same flags.
///
/// Prefer the aliases below over spelling the parameters out:
///
/// ```rust,ignore
/// #[pyfunction]
/// fn count_bytes(mut f: PyReadFile) -> PyResult<usize> {
///     let mut buf = Vec::new();
///     f.read_to_end(&mut buf)?; // `Read` is available because READ is true
///     Ok(buf.len())
/// }
/// ```
///
/// Only the requested traits are implemented, so using a method that was not asked for is caught at
/// compile time instead of failing as a Python `TypeError` at runtime:
///
/// ```compile_fail
/// use std::io::Read;
/// use pyo3_file::PyWriteFile;
///
/// fn read_from(f: &mut PyWriteFile, buf: &mut Vec<u8>) {
///     f.read_to_end(buf).unwrap(); // error: `PyWriteFile` does not implement `Read`
/// }
/// ```
///
/// ```rust
/// use std::io::{Read, Seek, Write};
/// use pyo3_file::{PyReadFile, PySeekableWriteFile};
///
/// fn assert_read<T: Read>() {}
/// fn assert_write_seek<T: Write + Seek>() {}
///
/// assert_read::<PyReadFile>();
/// assert_write_seek::<PySeekableWriteFile>();
/// ```
///
/// The wrapper is a convenience, not a boundary: it derefs to the inner [`PyFileLikeObject`], whose
/// inherent `py_*` methods remain callable regardless of the parameters. The [`Deref`] is
/// deliberately not paired with a `DerefMut`, which would otherwise hand out a
/// `&mut PyFileLikeObject` and with it every one of the `std::io` impls.
///
/// [`Deref`]: std::ops::Deref
#[derive(Debug, Clone)]
pub struct PyFileLike<const READ: bool, const WRITE: bool, const SEEK: bool, const FILENO: bool>(
    PyFileLikeObject,
);

/// A file-like object that must support `read`.
pub type PyReadFile = PyFileLike<true, false, false, false>;
/// A file-like object that must support `write`.
pub type PyWriteFile = PyFileLike<false, true, false, false>;
/// A file-like object that must support `read` and `seek`.
pub type PySeekableReadFile = PyFileLike<true, false, true, false>;
/// A file-like object that must support `write` and `seek`.
pub type PySeekableWriteFile = PyFileLike<false, true, true, false>;
/// A file-like object that must support `read`, `write` and `seek`.
pub type PySeekableReadWriteFile = PyFileLike<true, true, true, false>;

impl<const R: bool, const W: bool, const S: bool, const F: bool> PyFileLike<R, W, S, F> {
    /// Borrows the wrapped [`PyFileLikeObject`].
    pub fn as_inner(&self) -> &PyFileLikeObject {
        &self.0
    }

    /// Unwraps into the underlying [`PyFileLikeObject`], discarding the requirements.
    pub fn into_inner(self) -> PyFileLikeObject {
        self.0
    }
}

impl<const R: bool, const W: bool, const S: bool, const F: bool> std::ops::Deref
    for PyFileLike<R, W, S, F>
{
    type Target = PyFileLikeObject;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<const R: bool, const W: bool, const S: bool, const F: bool> From<PyFileLike<R, W, S, F>>
    for PyFileLikeObject
{
    fn from(value: PyFileLike<R, W, S, F>) -> Self {
        value.0
    }
}

/// The type hints this module can produce, as named constants so the `match` below reads as a
/// table.
#[cfg(feature = "experimental-inspect")]
mod hints {
    use pyo3::inspect::PyStaticExpr;
    use pyo3::{type_hint_identifier, type_hint_subscript, type_hint_union};

    const ANY: PyStaticExpr = type_hint_identifier!("typing", "Any");

    pub(super) const READS: PyStaticExpr =
        type_hint_subscript!(type_hint_identifier!("_typeshed", "SupportsRead"), ANY);
    pub(super) const WRITES: PyStaticExpr =
        type_hint_subscript!(type_hint_identifier!("_typeshed", "SupportsWrite"), ANY);
    pub(super) const READS_OR_WRITES: PyStaticExpr = type_hint_union!(READS, WRITES);
    pub(super) const HAS_FILENO: PyStaticExpr = type_hint_identifier!("_typeshed", "HasFileno");

    /// The fallback for requirements Python's type system cannot state.
    ///
    /// `io.IOBase` and `typing.IO` are both nominal, so neither accepts a plain duck-typed object,
    /// but between them they cover every file object in the standard library: `io.IOBase` catches
    /// `gzip.GzipFile` and custom `RawIOBase`/`BufferedIOBase` subclasses, which `typing.IO`
    /// rejects, and `typing.IO` catches `tempfile.NamedTemporaryFile`, which `io.IOBase` rejects.
    pub(super) const SOME_FILE_OBJECT: PyStaticExpr = type_hint_union!(
        type_hint_identifier!("io", "IOBase"),
        type_hint_subscript!(type_hint_identifier!("typing", "IO"), ANY)
    );
}

impl<'py, const R: bool, const W: bool, const S: bool, const F: bool> FromPyObject<'_, 'py>
    for PyFileLike<R, W, S, F>
{
    type Error = PyErr;

    /// The Python type this set of requirements is described by in generated type stubs.
    ///
    /// Where a requirement maps onto a single protocol, the hint is exact and structural, so
    /// duck-typed objects are accepted just as they are at runtime. Anything else falls back to
    /// [`hints::SOME_FILE_OBJECT`], because Python has no intersection type — "supports read *and*
    /// write" cannot be written down — and `_typeshed` has no `SupportsSeek`, so seek cannot be
    /// expressed structurally at all.
    ///
    /// Requiring nothing gives the same hint as a bare [`PyFileLikeObject`].
    #[cfg(feature = "experimental-inspect")]
    const INPUT_TYPE: pyo3::inspect::PyStaticExpr = match (R, W, S, F) {
        (false, false, false, false) => hints::READS_OR_WRITES,
        (true, false, false, false) => hints::READS,
        (false, true, false, false) => hints::WRITES,
        (false, false, false, true) => hints::HAS_FILENO,
        _ => hints::SOME_FILE_OBJECT,
    };

    fn extract(obj: Borrowed<'_, 'py, PyAny>) -> Result<Self, Self::Error> {
        PyFileLikeObject::py_with_requirements(obj.as_any().clone(), R, W, S, F).map(Self)
    }
}

impl<const W: bool, const S: bool, const F: bool> Read for PyFileLike<true, W, S, F> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
        Python::attach(|py| self.0.py_read(py, buf))
    }
}

impl<const W: bool, const S: bool, const F: bool> Read for &PyFileLike<true, W, S, F> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
        Python::attach(|py| self.0.py_read(py, buf))
    }
}

impl<const R: bool, const S: bool, const F: bool> Write for PyFileLike<R, true, S, F> {
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        Python::attach(|py| self.0.py_write(py, buf))
    }

    fn flush(&mut self) -> Result<(), io::Error> {
        Python::attach(|py| self.0.py_flush(py))
    }
}

impl<const R: bool, const S: bool, const F: bool> Write for &PyFileLike<R, true, S, F> {
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        Python::attach(|py| self.0.py_write(py, buf))
    }

    fn flush(&mut self) -> Result<(), io::Error> {
        Python::attach(|py| self.0.py_flush(py))
    }
}

impl<const R: bool, const W: bool, const F: bool> Seek for PyFileLike<R, W, true, F> {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, io::Error> {
        Python::attach(|py| self.0.py_seek(py, pos))
    }
}

impl<const R: bool, const W: bool, const F: bool> Seek for &PyFileLike<R, W, true, F> {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, io::Error> {
        Python::attach(|py| self.0.py_seek(py, pos))
    }
}

#[cfg(unix)]
impl<const R: bool, const W: bool, const S: bool> AsRawFd for PyFileLike<R, W, S, true> {
    fn as_raw_fd(&self) -> RawFd {
        Python::attach(|py| self.0.py_as_raw_fd(py))
    }
}

#[cfg(unix)]
impl<const R: bool, const W: bool, const S: bool> AsRawFd for &PyFileLike<R, W, S, true> {
    fn as_raw_fd(&self) -> RawFd {
        Python::attach(|py| self.0.py_as_raw_fd(py))
    }
}

#[cfg(all(test, feature = "experimental-inspect"))]
mod tests {
    use super::*;

    fn hint<const R: bool, const W: bool, const S: bool, const F: bool>() -> String {
        <PyFileLike<R, W, S, F> as FromPyObject>::INPUT_TYPE.to_string()
    }

    #[test]
    fn single_requirements_get_an_exact_structural_hint() {
        assert_eq!(
            hint::<true, false, false, false>(),
            "_typeshed.SupportsRead[typing.Any]"
        );
        assert_eq!(
            hint::<false, true, false, false>(),
            "_typeshed.SupportsWrite[typing.Any]"
        );
        assert_eq!(hint::<false, false, false, true>(), "_typeshed.HasFileno");
    }

    #[test]
    fn no_requirements_matches_a_bare_file_like_object() {
        assert_eq!(
            hint::<false, false, false, false>(),
            <PyFileLikeObject as FromPyObject>::INPUT_TYPE.to_string()
        );
    }

    #[test]
    fn inexpressible_requirements_fall_back_to_a_file_object() {
        let fallback = "io.IOBase | typing.IO[typing.Any]";
        // seek has no structural equivalent
        assert_eq!(hint::<false, false, true, false>(), fallback);
        assert_eq!(hint::<true, false, true, false>(), fallback);
        // read *and* write would need an intersection type
        assert_eq!(hint::<true, true, false, false>(), fallback);
        assert_eq!(hint::<true, true, true, true>(), fallback);
    }
}
