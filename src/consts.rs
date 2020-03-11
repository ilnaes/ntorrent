use std::time::Duration;

pub const TIMEOUT: Duration = Duration::from_secs(10);
pub const BLOCKSIZE: usize = 1 << 14;
pub const MAXREQUESTS: usize = 1;
