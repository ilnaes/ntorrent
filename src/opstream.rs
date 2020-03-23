use crate::messages::messages::Message;
use byteorder::{BigEndian, WriteBytesExt};
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

    pub fn close(&mut self) {
        self.stream.take();
    }

    pub async fn read_message(&mut self) -> Option<Message> {
        if let Some(s) = &mut self.stream {
            let len = timeout(consts::TIMEOUT, s.read_u32()).await.ok()?.ok()? as usize;
            print!("Reading {} bytes -- ", len);
            if len > consts::BLOCKSIZE as usize + 1000 {
                panic!("{}", len);
            }

            if len == 0 {
                return Some(Message::KeepAlive);
            }

            let mut buf = vec![0; len];
            print!("BEFORE ");
            timeout(consts::TIMEOUT, s.read_exact(&mut buf)).await.ok()?.ok()?;
            print!("AFTER ");
            Message::deserialize(buf)
        } else {
            None
        }
    }

    pub async fn send_message(&mut self, m: Message) -> Option<()> {
        // println!("Sending {:?}", m);
        let msg = m.serialize();
        // self.file.write_all(format!("{:?}\n", msg).as_bytes());
        if let Some(s) = &mut self.stream {
            timeout(consts::TIMEOUT, s.write_all(msg.as_slice())).await.ok()?.ok()?;
            return Some(())
        }
        None
    }
}
