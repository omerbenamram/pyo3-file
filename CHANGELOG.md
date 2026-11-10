# Changelog
Please see https://github.com/omerbenamram/pyo3-file/releases

## Unreleased

- `as_raw_fd` and `py_as_raw_fd` panicked for an object without a file
  descriptor, such as an `io.BytesIO`, whose `fileno()` raises
  `io.UnsupportedOperation`. `AsRawFd::as_raw_fd` can't return an error, so
  they now return `-1`, which no system call accepts, when `fileno()` is
  missing, raises, or doesn't return a descriptor. The new `try_as_raw_fd` and
  `py_try_as_raw_fd` return the descriptor or the error that says why there is
  none.
