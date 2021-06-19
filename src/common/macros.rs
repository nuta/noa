#[macro_export]
macro_rules! warn_on_error {
    ($expr:expr, $context:expr) => {
        if let Err(err) = $expr {
            warn!("{}: {}", $context, err);
        }
    };
}

#[macro_export]
macro_rules! debug_feature_frag {
    ($id:ident) => {
        cfg!(debug_assertions) && ::std::env::var(concat!("NOA_", stringify!($id))).is_ok()
    };
}
