# reportable

Declare who should receive an error's diagnostic details: your operators or the caller.

`reportable` provides `ReportTo`, the `Reportable` trait, and a derive macro for nested
error enums. It works alongside `thiserror` and does not depend on a logging framework,
HTTP library, or error tracker. The runtime crate supports `no_std` without allocation.

## Installation

Requires Rust 1.85 or later. Add `reportable = "0.1"` to your dependencies.
The derive macro is included. `thiserror = "2"` is optional and used in the example below.

## Usage

```rust
use reportable::{ReportTo, Reportable};

#[derive(Debug, thiserror::Error, Reportable)]
enum RepositoryError {
    #[error("Record not found")]
    #[reportable(caller)]
    NotFound,

    #[error("Database unavailable")]
    #[reportable(internal)]
    Unavailable,
}

#[derive(Debug, thiserror::Error, Reportable)]
enum UpdateError {
    #[error("Invalid input: {0}")]
    #[reportable(caller)]
    InvalidInput(String),

    #[error(transparent)]
    #[reportable(transparent)]
    Repository(#[from] RepositoryError),

    #[error("Required record missing: {0}")]
    #[reportable(internal)]
    Invariant(RepositoryError),
}

let error = UpdateError::from(RepositoryError::NotFound);
assert_eq!(error.report_to(), ReportTo::Caller);

let error = UpdateError::Invariant(RepositoryError::NotFound);
assert_eq!(error.report_to(), ReportTo::Internal);
```

## Classification rules

- `#[reportable(caller)]`: an expected failure for the caller to handle.
- `#[reportable(internal)]`: a failure for operators to investigate.
- `#[reportable(transparent)]`: delegate to the wrapped field's `Reportable` implementation.

Every variant needs exactly one annotation. There is no implicit or enum-level default.
Explicit classifications accept unit, tuple, and named-field variants and do not require
their fields to implement `Reportable`. Transparent variants require exactly one field,
named or unnamed, whose type implements `Reportable`. Generic bounds are generated for
delegated field types. References to `Reportable` values also implement the trait.

Classification describes the destination of diagnostic details. An internal failure can
still return a generic error response to the caller. HTTP status and retry behavior are
separate decisions. A parent can explicitly classify a wrapped error differently when
its context changes the meaning of that failure.

Implement `Reportable` manually if the destination depends on a field's value.
The trait does not require `std::error::Error`.

## Reporting with tracing and Sentry

The application owns reporting. With `tracing` and `sentry-tracing`, classify the typed
error before passing it to the logging framework:

```rust,ignore
use reportable::{ReportTo, Reportable};
use std::error::Error;

fn report_error<E: Error + Reportable + 'static>(error: &E) {
    match error.report_to() {
        ReportTo::Caller => tracing::info!(error = %error),
        ReportTo::Internal => tracing::error!(error = error as &dyn Error),
    }
}
```

The [Sentry tracing layer](https://docs.rs/sentry-tracing/latest/sentry_tracing/)
captures `ERROR` events by default; expected errors logged at
`INFO` do not create issues. They may become breadcrumbs depending on layer configuration.
Passing `&dyn Error` preserves the error's source chain.

Call this once where an operation finishes: an Actix `ResponseError::error_response`
implementation or a worker's job handler. Lower layers should return errors without also
reporting them. Avoid a second capture through `#[instrument(err)]`, middleware, or
`sentry::capture_error` on the same path. Retry policy belongs to the application.

## Compile-time checks

An unclassified variant is rejected:

```compile_fail
use reportable::Reportable;

#[derive(Reportable)]
enum Error {
    Unclassified,
}
```

Delegation requires a `Reportable` field:

```compile_fail
use reportable::Reportable;

#[derive(Reportable)]
enum Error {
    #[reportable(transparent)]
    Foreign(std::io::Error),
}
```

Transparent variants cannot silently select one of several fields:

```compile_fail
use reportable::Reportable;

#[derive(Reportable)]
enum Error {
    #[reportable(transparent)]
    Ambiguous(u8, u8),
}
```

## Development

```sh
cargo test --workspace --locked
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo doc --workspace --no-deps --locked
```

CI tests stable Rust and Rust 1.85, checks a target without `std`, and verifies both
crate archives. See the repository's `PUBLISHING.md` for release steps.

## License

MIT.
