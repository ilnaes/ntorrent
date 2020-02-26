use crate::torrents::Torrent;
use crate::consts;
use crate::err::ConnectError;
use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio::prelude::*;
use std::error::Error;
use byteorder::{BigEndian, WriteBytesExt, ReadBytesExt};

#[derive(Serialize, Deserialize, Debug)]
pub struct TrackerResponse {
    pub interval: u64,
    pub peers: ByteBuf,
}

pub struct Handshake {
    pub info_hash: Vec<u8>,
    pub peer_id: Vec<u8>,
}

impl Handshake {
    pub fn from(t: &Torrent) -> Handshake {
        Handshake {
            info_hash: t.info_hash.clone(),
            peer_id: t.peer_id.clone(),
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut res = vec![19u8];
        res.extend("BitTorrent protocol".as_bytes());
        res.extend(vec![0; 8]);
        res.extend(self.info_hash.clone());
        res.extend(self.peer_id.clone());
        res
    }

    pub fn deserialize(msg: Vec<u8>) -> Option<Handshake> {
        let buf = msg.as_slice();
        if buf[0] != 19 {
            return None;
        }

        if let Ok(s) = String::from_utf8(buf[1..20].to_vec()) {
            if s != "BitTorrent protocol" {
                return None;
            }
        } else {
            return None;
        }

        Some(Handshake {
            info_hash: buf[28..48].to_vec(),
            peer_id: buf[48..68].to_vec(),
        })
    }
}

#[derive(PartialEq, Debug)]
pub enum MessageID {
    KeepAlive,
    Choke,
    Unchoke,
    Bitfield,
    Interested,
    NotInterested,
    Request,
    Have,
    Piece,
    Cancel,
    Port,
}

#[derive(Debug)]
pub struct Message {
    pub message_id: MessageID,
    pub payload: Option<Vec<u8>>,
}

impl Message {
    pub fn write_id(id: &MessageID) -> u8 {
        match id {
            MessageID::KeepAlive => 0,
            MessageID::Choke => 0,
            MessageID::Unchoke => 1,
            MessageID::Interested => 2,
            MessageID::NotInterested => 3,
            MessageID::Have => 4,
            MessageID::Bitfield => 5,
            MessageID::Request => 6,
            MessageID::Piece => 7,
            MessageID::Cancel => 8,
            MessageID::Port => 9,
        }
    }

    pub fn get_id(u: u8) -> Result<MessageID, Box<dyn Error>> {
        match u {
            0 => Ok(MessageID::Choke),
            1 => Ok(MessageID::Unchoke),
            2 => Ok(MessageID::Interested),
            3 => Ok(MessageID::NotInterested),
            4 => Ok(MessageID::Have),
            5 => Ok(MessageID::Bitfield),
            6 => Ok(MessageID::Request),
            7 => Ok(MessageID::Piece),
            8 => Ok(MessageID::Cancel),
            9 => Ok(MessageID::Port),
            _ => Err(Box::new(ConnectError)),
        }
    }

    pub async fn read_from(s: &mut TcpStream) -> Result<Message, Box<dyn Error>> {
        let len: usize = timeout(consts::TIMEOUT, s.read_u32()).await?? as usize;

        if len == 0 {
            return  Ok(Message { message_id: MessageID::KeepAlive, payload: None })
        }

        let mut buf = vec![0; len];
        timeout(consts::TIMEOUT, s.read_exact(&mut buf)).await??;
        Ok(Message{
            message_id: Message::get_id(buf[0])?,
            payload: Some(buf.drain(1..).collect()),
        })
    }

    pub fn serialize(&self) -> Vec<u8> {
        if self.message_id == MessageID::KeepAlive {
            return vec![0;4]
        }

        let len = if let Some(v) = &self.payload {
            5 + v.len()
        } else {
            5
        };

        let mut res = vec![];
        WriteBytesExt::write_u32::<BigEndian>(&mut res, len as u32).unwrap(); 
        WriteBytesExt::write_u8(&mut res, Message::write_id(&self.message_id)).unwrap(); 
        if let Some(v) = &self.payload {
            res.extend(v);
        }
        res
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_serialize() {
        let m = super::Message{
            message_id: super::MessageID::Have,
            payload: Some(vec![1;4]),
        };

        assert_eq!(m.serialize(), vec![0,0,0,9,4,1,1,1,1]);
    }
}
