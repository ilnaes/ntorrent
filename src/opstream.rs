use crate::consts;
use crate::messages::messages::Message;
use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_util::codec::{Framed, LengthDelimitedCodec};

pub struct OpStream {
    stream: Option<Framed<TcpStream, LengthDelimitedCodec>>,
}

impl OpStream {
    pub fn new() -> OpStream {
        OpStream { stream: None }
    }

    pub fn from(s: TcpStream) -> OpStream {
        OpStream {
            stream: Some(Framed::new(s, LengthDelimitedCodec::new())),
        }
    }

    pub fn close(&mut self) {
        self.stream.take();
    }

    pub async fn read_message(&mut self) -> Option<Message> {
        if let Some(s) = &mut self.stream {
            let buf = timeout(consts::TIMEOUT, s.next()).await.ok()??.ok()?;
            if buf.len() == 0 {
                return Some(Message::KeepAlive);
            }
            return Message::deserialize(buf.as_ref());
        }
        None
    }

    pub async fn send_message(&mut self, m: Message) -> Option<()> {
        let msg = m.serialize();
        if let Some(s) = &mut self.stream {
            if let Err(e) = timeout(consts::TIMEOUT, s.send(Bytes::from(msg)))
                .await
                .ok()?
            {
                eprintln!("Error writing: {}", e);
                return None;
            }
            return Some(());
        }
        None
    }
}
