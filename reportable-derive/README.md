# reportable-derive

Procedural macro implementation for [reportable](https://docs.rs/reportable).

Use `#[derive(Reportable)]` through the `reportable` crate, which re-exports the macro
alongside the trait and `ReportTo` enum. Each enum variant must be annotated with
`#[reportable(caller)]`, `#[reportable(internal)]`, or `#[reportable(transparent)]`.

Requires Rust 1.85 or later. Add `reportable = "0.1"` to your dependencies;
applications do not need to depend on this crate directly.

Licensed under MIT.
