/*
    Packet fields: Action | ChunkID | Len Filename | Len ChunkData | Filename |   ChunkData  |
    Bytes fields:    1    |    4    |      1       |       2      |   25max  |    1420max   |  TOTAL 1453 bytes MAX

    IPv4 Header 20 bytes
    UDP header 8 bytes
    AVAILABLE 1500-20-8 = 1472

    1472-1453 = 19 bytes spare

    Action:
            0 - ACK Reply
            1 - Chunk Request
            2 - Chunk Reply
*/

#![allow(dead_code)]

use anyhow::bail;

pub const MAX_PACKET_SIZE: usize = 1453;
pub const MAX_CHUNK_SIZE: usize = 1420;
pub const SERVER_UDP_PORT: usize = 9090;

#[derive(Debug)]
pub struct Protocol<'a> {
    pub action: u8,
    pub chunk_id: u32,
    pub filename: &'a str,
    pub len_chunk: u16,
    pub chunk_data: [u8; MAX_CHUNK_SIZE],
}

impl<'a> Protocol<'a> {
    pub fn read_packet(
        packet: &'a [u8],
        length: u16,
    ) -> anyhow::Result<Protocol> {
        if length < 8 {
            bail!("Invalid payload, not enough bytes");
        }

        let mut field2: [u8; 4] = [0; 4];
        field2.copy_from_slice(&packet[1..5]);

        let len_filename: u8 = packet[5];
        let mut field3: [u8; 2] = [0; 2];
        field3.copy_from_slice(&packet[6..8]);
        let len_c: u16 = u16::from_le_bytes(field3);

        let byte_chunk_data: usize = 8 + len_filename as usize;

        let mut data: [u8; MAX_CHUNK_SIZE] = [0; MAX_CHUNK_SIZE];
        let mut i: usize = 0;
        for byte in &packet[byte_chunk_data..(byte_chunk_data + len_c as usize)]
        {
            data[i] = byte.clone();
            i += 1;
        }

        match std::str::from_utf8(&packet[8..byte_chunk_data]) {
            Ok(f) => Ok(Protocol {
                action: packet[0],
                chunk_id: u32::from_le_bytes(field2),
                len_chunk: len_c,
                filename: f,
                chunk_data: data,
            }),
            Err(_) => bail!("Failed parsing filename"),
        }
    }

    pub fn build_packet(&self) -> Option<([u8; MAX_PACKET_SIZE], u16)> {
        if self.filename.len() > 25 || self.chunk_data.len() > 1420 {
            return None;
        }

        let action: [u8; 1] = self.action.to_le_bytes();
        let chunk_id: [u8; 4] = self.chunk_id.to_le_bytes();
        let len_filename: [u8; 1] = (self.filename.len() as u8).to_le_bytes();
        let len_chunk_data: [u8; 2] = self.len_chunk.to_le_bytes();
        let filename: &[u8] = self.filename.as_bytes();

        let packet_len: u16 = 1
            + 4
            + 1
            + 2
            + self.filename.len() as u16
            + self.chunk_data.len() as u16;
        let mut packet: [u8; MAX_PACKET_SIZE] = [0; MAX_PACKET_SIZE];

        packet[0] = action[0];
        packet[1..=4].copy_from_slice(&chunk_id);
        packet[5] = len_filename[0];
        packet[6..=7].copy_from_slice(&len_chunk_data);
        packet[8..=8 - 1 + len_filename[0] as usize].copy_from_slice(&filename);

        if self.len_chunk > 0 {
            packet[8 + len_filename[0] as usize
                ..=8 - 1 + len_filename[0] as usize + self.len_chunk as usize]
                .copy_from_slice(&self.chunk_data[0..self.len_chunk as usize]);
        }

        Some((packet, packet_len))
    }

    pub fn clone(&self) -> Protocol {
        let mut data: [u8; MAX_CHUNK_SIZE] = [0; MAX_CHUNK_SIZE];

        let mut i: usize = 0;
        for byte in self.chunk_data {
            data[i] = byte.clone();
            i += 1;
        }

        Protocol {
            action: self.action,
            chunk_id: self.chunk_id,
            filename: self.filename,
            len_chunk: self.len_chunk,
            chunk_data: data,
        }
    }

    pub fn to_string(&self) -> String{
        return "Action:".to_string()+&self.action.to_string()+
            " ID:"+
            &self.chunk_id.to_string()+
            " Filename:"+
            self.filename+
            " LenChunk:"+&self.len_chunk.to_string()+
            "\n"+&String::from_utf8(self.chunk_data[0..self.len_chunk as usize].to_vec()).unwrap();
    }
}

impl<'a> PartialEq for Protocol<'a>{
    fn eq(&self,other:&Self) -> bool{
        self.action == other.action &&
        self.chunk_id == other.chunk_id &&
        self.filename == other.filename &&
        self.len_chunk == other.len_chunk &&
        self.chunk_data[0..self.len_chunk as usize] == other.chunk_data[0..other.len_chunk as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fsnp_test() {
        let payload: Protocol = Protocol {
            action: 1,
            chunk_id: 69,
            filename: "dingo",
            len_chunk: 0,
            chunk_data: [0; MAX_CHUNK_SIZE],
        };

        let (serialized, len) = payload.build_packet().unwrap();
        let parsed: Protocol = Protocol::read_packet(&serialized, len).unwrap();

        assert_eq!(payload.action, parsed.action);
        assert_eq!(payload.chunk_id, parsed.chunk_id);
        assert_eq!(payload.filename, parsed.filename);
        assert_eq!(payload.len_chunk, parsed.len_chunk);
        assert_eq!(payload.chunk_data, parsed.chunk_data);
    }
}
