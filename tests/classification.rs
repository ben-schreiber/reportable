use reportable::{ReportTo, Reportable};

#[derive(Debug, thiserror::Error, Reportable)]
enum RepositoryError {
    #[error("Record not found")]
    #[reportable(caller)]
    NotFound,
    #[error("Connection failed: {0}")]
    #[reportable(internal)]
    Connection(#[from] std::io::Error),
}

#[derive(Debug, thiserror::Error, Reportable)]
enum UpdateError {
    #[error(transparent)]
    #[reportable(transparent)]
    Repository(#[from] RepositoryError),
    #[error("Required record missing: {source}")]
    #[reportable(internal)]
    Invariant { source: RepositoryError },
    #[error("Invalid input: {0}")]
    #[reportable(caller)]
    InvalidInput(String),
}

#[derive(Reportable)]
enum Envelope<T> {
    #[reportable(transparent)]
    Nested { source: T },
}

#[test]
fn delegates_through_nested_errors_and_respects_parent_overrides() {
    let missing = UpdateError::from(RepositoryError::NotFound);
    let failure = UpdateError::from(RepositoryError::from(std::io::Error::other("offline")));
    assert_eq!(
        Envelope::Nested { source: missing }.report_to(),
        ReportTo::Caller
    );
    assert_eq!(
        Envelope::Nested { source: failure }.report_to(),
        ReportTo::Internal
    );
    assert_eq!(
        UpdateError::Invariant {
            source: RepositoryError::NotFound
        }
        .report_to(),
        ReportTo::Internal,
    );
    assert_eq!(
        UpdateError::InvalidInput("name".into()).report_to(),
        ReportTo::Caller
    );
}

#[derive(Reportable)]
enum Borrowed<'a, T: ?Sized, const N: usize>
where
    T: Reportable,
{
    #[reportable(transparent)]
    Inner(&'a T),
    #[reportable(caller)]
    Input { _bytes: [u8; N] },
}

#[derive(Reportable)]
enum Opaque<T> {
    #[reportable(internal)]
    Value(T),
}

#[derive(Reportable)]
enum Empty {}

#[test]
fn preserves_generics_and_does_not_require_classification_of_explicit_payloads() {
    let error: &dyn Reportable = &RepositoryError::NotFound;
    let borrowed = Borrowed::<'_, dyn Reportable, 4>::Inner(error);
    assert_eq!(borrowed.report_to(), ReportTo::Caller);
    let input: Borrowed<'_, dyn Reportable, 4> = Borrowed::Input { _bytes: [0; 4] };
    assert_eq!(input.report_to(), ReportTo::Caller);
    assert_eq!(
        Opaque::Value(std::io::Error::other("opaque")).report_to(),
        ReportTo::Internal
    );
    fn assert_reportable<T: Reportable>() {}
    assert_reportable::<Empty>();
}
