use crate::messages::Message;
use crate::consts;
use crate::err::ConnectError;
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio::prelude::*;

pub struct OpStream {
    stream: Option<TcpStream>,
}

impl OpStream {
    pub fn new() -> OpStream {
        OpStream { stream: None }
    }

    pub fn from(s: TcpStream) -> OpStream {
        OpStream { stream: Some(s) }
    }

    pub async fn read_message(&mut self) -> Option<Message> {
        if let Some(s) = &mut self.stream {
            return Message::read_from(s).await.ok()
        } else {
            None
        }
    }

    pub async fn send_message(&mut self, m: Message) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(s) = &mut self.stream {
            timeout(consts::TIMEOUT, s.write_all(m.serialize().as_slice())).await??;
            return Ok(())
        }
        Err(Box::new(ConnectError))
    }
}
