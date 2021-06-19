#[macro_export]
macro_rules! warn_on_error {
    ($expr:expr, $context:expr) => {
        if let Err(err) = $expr {
            warn!("{}: {}", $context, err);
        }
    };
}
