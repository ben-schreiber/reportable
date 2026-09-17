# reportable-derive

Procedural macro implementation for `reportable`.

Use `#[derive(Reportable)]` through the `reportable` crate, which re-exports the macro
alongside the trait and `ReportTo` enum. Each enum variant must be annotated with
`#[reportable(caller)]`, `#[reportable(internal)]`, or `#[reportable(transparent)]`.

Licensed under MIT.
