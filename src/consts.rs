use std::time::Duration;

pub const TIMEOUT: Duration = Duration::from_secs(10);
pub const BLOCKSIZE: u32 = 1 << 14;
pub const MAXREQUESTS: u32 = 5;
