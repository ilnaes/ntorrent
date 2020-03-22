use crate::messages::messages::{Message, MessageID};
use crate::consts;
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
            let len: usize = timeout(consts::TIMEOUT, s.read_u32()).await.ok()?.ok()? as usize;

            if len == 0 {
                return  Some(Message { message_id: MessageID::KeepAlive, payload: None })
            }

            let mut buf = vec![0; len];
            timeout(consts::TIMEOUT, s.read_exact(&mut buf)).await.ok()?.ok()?;
            Some(Message{
                message_id: Message::get_id(buf[0])?,
                payload: Some(buf.drain(1..).collect()),
            })
        } else {
            None
        }
    }

    pub async fn send_message(&mut self, m: Message) -> Option<()> {
        if let Some(s) = &mut self.stream {
            timeout(consts::TIMEOUT, s.write_all(m.serialize().as_slice())).await.ok()?.ok()?;
            return Some(())
        }
        None
    }
}
