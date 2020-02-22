use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;

#[derive(Serialize, Deserialize, Debug)]
pub struct TrackerResponse {
    pub interval: u64,
    pub peers: ByteBuf,
}
