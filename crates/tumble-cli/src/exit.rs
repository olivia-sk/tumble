//! Exit codes (PRD section 10).

/// Everything succeeded.
pub const OK: u8 = 0;
/// At least one file failed to convert.
pub const SOME_FAILED: u8 = 1;
/// The command line was wrong.
pub const BAD_ARGS: u8 = 2;
/// No route for the request, or the engine it needs is missing.
pub const NO_ROUTE: u8 = 3;
