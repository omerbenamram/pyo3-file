# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Breaking Changes

- `PyFileLikeObject::new` and `PyFileLikeObject::require` now take a `&PyAny`
  instead of a `PyObject`

### Added

- `PyFileLikeObject` now implements `FromPyObject`/`ToPyObject`, which allows
  it to be used directly as an argument to a pyfunction or as a return value
- New constructor `PyFileLikeObject::with_rw_seek` which takes const generic
  arguments to specify whether the object should be readable, writable and
  seekable, which can be used in `#[pyo3(from_py_with)]` when declaring the
  pyfunction or in a type deriving `FromPyObject`
- New functions `PyFileLikeObject::check_readable`,
  `PyFileLikeObject::check_writable` and `PyFileLikeObject::check_seekable`
  which can be used to check whether the object is readable, writable or
  seekable, respectively.
- `io::Error`s returned by the Read/Write/Seek implementations for
  `PyFileLikeObject` now pass along the original python exception, and attempt
  to set the error kind based on the `errno` field of the python exception, if
  present.
- `Read`, `Write`, and `Seek` are now additionally implemented for
  `&PyFileLikeObject` since they use the GIL for interior mutability.
- `PyFileLikeObject` now implements `Clone`, and provides a `clone_ref` method
  which can be used to clone the object more efficiently when already holding
  the GIL.
- New function `PyFileLikeObject::fileno` which attempts to call the fileno
  method on the underlying python object, and returns the result as a `i64` if
  successful.
- Add support for pyo3 0.20

### Fixes

- The `Read` implementation for `PyFileLikeObject` will now return errors
  rather than panicking if the underlying python object returns an unexpected
  type from the `read` python method.
- The `Write` implementation for `PyFileLikeObject` will now return errors
  rather than panicking when attempting to write non-utf8 bytes to a textio
  object.


## [0.7.0] - 2023-06-26

- Switch to GH actions by @omerbenamram in #3
- Add textio support to PyFileLikeObject by @ethanhs in #4
- Divide actually used buffer size by 4 for TextIO by @smheidrich in #7
- Add support for pyo3 0.19 by @jelmer in #11
- Improve error message when write() returns None by @jelmer in #10
- Added a crates io publishing workflow for tags by @ohadravid in #12

## [0.5.0] - 2022-04-16

- Update to PyO3 0.16
- Add textio support (thanks @ethanhs)
- Remove `Cargo.lock` (as this is a library)

## [0.4.0] - 2021-07-09

Update to PyO3 0.14

## [0.3.0] - 2019-10-30

Update to PyO3 0.8.2

## [0.2.0] - 2019-05-20

Includes a minor breaking change to constructor behavior.

### Changed
- Added another constructor `PyFileLikeObject::require` that validates the object has the required method,
 `PyFileLikeObject::new` now cannot fail.

## [0.1.0] - 2019-05-20

Initial release
