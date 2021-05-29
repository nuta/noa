#[macro_export]
macro_rules! warn_on_error {
    ($context:expr, $expr:expr) => {
        if let Err(err) = $expr {
            warn!("{}: {}", $context, err);
        }
    };
}
