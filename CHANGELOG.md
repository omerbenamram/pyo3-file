# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.0] - 2019-10-30

Update to PyO3 0.8.2

## [0.2.0] - 2019-05-20

Includes a minor breaking change to constructor behavior.

### Changed
- Added another constructor `PyFileLikeObject::require` that validates the object has the required method,
 `PyFileLikeObject::new` now cannot fail. 

## [0.1.0] - 2019-05-20

Initial release