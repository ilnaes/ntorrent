use byteorder::{BigEndian, WriteBytesExt};
use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;

#[derive(Serialize, Deserialize, Debug)]
pub struct TrackerResponse {
    pub interval: u64,
    pub peers: ByteBuf,
}

#[derive(PartialEq, Debug, Clone)]
pub enum MessageID {
    KeepAlive,
    Choke,
    Unchoke,
    Interested,
    NotInterested,
    Bitfield,
    Request,
    Have,
    Piece,
    Cancel,
    Port,
}

#[derive(Debug, Clone, PartialEq)]
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

    pub fn get_id(u: u8) -> Option<MessageID> {
        match u {
            0 => Some(MessageID::Choke),
            1 => Some(MessageID::Unchoke),
            2 => Some(MessageID::Interested),
            3 => Some(MessageID::NotInterested),
            4 => Some(MessageID::Have),
            5 => Some(MessageID::Bitfield),
            6 => Some(MessageID::Request),
            7 => Some(MessageID::Piece),
            8 => Some(MessageID::Cancel),
            9 => Some(MessageID::Port),
            _ => None,
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        if self.message_id == MessageID::KeepAlive {
            return vec![0; 4];
        }

        let len = if let Some(v) = &self.payload {
            1 + v.len()
        } else {
            1
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
        let m = super::Message {
            message_id: super::MessageID::Have,
            payload: Some(vec![1; 4]),
        };

        assert_eq!(m.serialize(), vec![0, 0, 0, 9, 4, 1, 1, 1, 1]);
    }
}
