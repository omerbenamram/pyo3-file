use pyo3::exceptions::{IOError, NotImplementedError, OSError, RuntimeError, ValueError};
use pyo3::prelude::*;
use pyo3::types::IntoPyDict;
use pyo3::types::{PyAny, PyBytes, PyDict};
use pyo3::wrap_pyfunction;
use pyo3::AsPyPointer;
use pyo3::PyIterProtocol;

use core::borrow::{Borrow, BorrowMut};
use std::convert::TryInto;
use std::fs::File;
use std::io;
use std::io::{Read, Seek, SeekFrom, Write};

#[derive(Debug)]
pub struct PyFileLikeObject {
    inner: PyObject,
}

impl PyFileLikeObject {
    pub fn new(inner: PyObject) -> Self {
        Self { inner }
    }
}

fn pyerr_to_io_err(e: PyErr) -> io::Error {
    let gil = Python::acquire_gil();
    let py = gil.python();

    match e.into_object(py).call_method(py, "__str__", (), None) {
        Ok(repr) => match repr.extract::<String>(py) {
            Ok(s) => io::Error::new(io::ErrorKind::Other, s),
            Err(e) => io::Error::new(io::ErrorKind::Other, "An unknown error has occurred"),
        },
        Err(_) => io::Error::new(io::ErrorKind::Other, "Err doesn't have __str__"),
    }
}

impl Read for PyFileLikeObject {
    fn read(&mut self, mut buf: &mut [u8]) -> Result<usize, io::Error> {
        let gil = Python::acquire_gil();
        let py = gil.python();

        let bytes = self
            .inner
            .call_method(py, "read", (buf.len(),), None)
            .map_err(pyerr_to_io_err)?;

        let bytes: &PyBytes = bytes
            .cast_as(py)
            .expect("Expecting to be able to downcast into bytes from read result.");

        &buf.write(bytes.as_bytes())?;

        Ok(bytes.len().map_err(pyerr_to_io_err)?)
    }
}

impl Write for PyFileLikeObject {
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        let gil = Python::acquire_gil();
        let py = gil.python();

        let pybytes = PyBytes::new(py, buf).into_object(py);

        let number_bytes_written = self
            .inner
            .call_method(py, "write", (pybytes,), None)
            .map_err(pyerr_to_io_err)?;

        Ok(number_bytes_written.extract(py).map_err(pyerr_to_io_err)?)
    }

    fn flush(&mut self) -> Result<(), io::Error> {
        let gil = Python::acquire_gil();
        let py = gil.python();

        self.inner
            .call_method(py, "flush", (), None)
            .map_err(pyerr_to_io_err)?;

        Ok(())
    }
}

impl Seek for PyFileLikeObject {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, io::Error> {
        let gil = Python::acquire_gil();
        let py = gil.python();

        let (whence, offset) = match pos {
            SeekFrom::Start(i) => (0, i as i64),
            SeekFrom::Current(i) => (1, i as i64),
            SeekFrom::End(i) => (2, i as i64),
        };

        dbg!("try_seek", offset, whence);
        let new_position = self
            .inner
            .call_method(py, "seek", (offset, whence), None)
            .map_err(pyerr_to_io_err)?;

        dbg!(new_position.extract::<u64>(py).unwrap());
        Ok(new_position.extract(py).map_err(pyerr_to_io_err)?)
    }
}
