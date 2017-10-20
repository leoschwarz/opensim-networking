//! Helpers to be used in tests.

use slog::{Drain, Logger};

pub fn get_logger() -> Logger {
    let decorator = ::slog_term::TermDecorator::new().build();
    let drain = ::slog_term::FullFormat::new(decorator).build().fuse();
    let drain = ::slog_async::Async::new(drain).build().fuse();
    Logger::root(drain, o!())
}
