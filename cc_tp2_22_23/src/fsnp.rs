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

        let byte_chunk_data: usize = 8 + len_filename as usize + 1;

        let mut data: [u8; MAX_CHUNK_SIZE] = [0; MAX_CHUNK_SIZE];
        let mut i: usize = 0;
        for byte in
            &packet[byte_chunk_data..(byte_chunk_data + len_c as usize + 1)]
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
        let len_chunk_data: [u8; 2] =
            (self.chunk_data.len() as u16).to_le_bytes();
        let filename: &[u8] = self.filename.as_bytes();

        let packet_len: u16 = 1
            + 4
            + 1
            + 2
            + self.filename.len() as u16
            + self.chunk_data.len() as u16;
        let mut packet: [u8; MAX_PACKET_SIZE] = [0; MAX_PACKET_SIZE];

        overwrite_array(0, 0, &action, &mut packet);
        overwrite_array(1, 4, &chunk_id, &mut packet);
        overwrite_array(5, 5, &len_filename, &mut packet);
        overwrite_array(6, 7, &len_chunk_data, &mut packet);
        overwrite_array(8, 8 + len_filename.len() - 1, &filename, &mut packet);
        overwrite_array(
            8 + len_filename.len(),
            8 + len_filename.len() + self.chunk_data.len() - 1,
            &self.chunk_data,
            &mut packet,
        );

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
}

pub fn overwrite_array(
    index_ini: usize,
    index_fini: usize,
    src: &[u8],
    dest: &mut [u8],
) {
    for i in index_ini..index_fini + 1 {
        dest[i] = src[i];
    }
}
