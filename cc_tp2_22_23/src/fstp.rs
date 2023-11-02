#![allow(dead_code)]

use anyhow::bail;

//TODO: Cenas de DNS
#[derive(Debug)]
pub struct FstpMessage<'a> {
    pub header: FstpHeader,
    pub data: Option<&'a [u8]>,
}

#[derive(Debug)]
pub struct FstpHeader {
    pub flag: Flag,
    pub data_size: u16,
}

#[derive(Debug)]
pub enum Flag {
    Ok,
    Add,
    List,
    File,
}

impl<'a> FstpMessage<'a> {
    pub fn put_in_bytes(self, buf: &mut [u8]) -> anyhow::Result<()> {
        let flag = &self.header.flag;
        let d_s = self.header.data_size;
        buf[0] = flag.to_bytes_flag();
        buf[1] = (d_s >> 8) as u8;
        buf[2] = d_s as u8;
        if let Some(data) = self.data {
            for i in 0..data.len() {
                buf[i + 3] = data[i];
            }
        }
        Ok(())
    }

    pub fn from_bytes(bytes: &[u8]) -> anyhow::Result<FstpMessage> {
        let (header, data) = bytes.split_at(3);
        let flag = Flag::from_bytes_flag(&header[0])?;
        let data_size = ((header[1] as u16) << 8) | header[2] as u16;
        let data = if data_size == 0 {
            None
        } else {
            Some(&data[0..(data_size as usize)])
        };
        Ok(FstpMessage {
            header: FstpHeader { flag, data_size },
            data,
        }) // return implicito
    }
}

impl Flag {
    fn to_bytes_flag(&self) -> u8 {
        match self {
            Self::Ok => 1u8,
            Self::Add => 2u8,
            Self::List => 3u8,
            Self::File => 4u8,
            // Self::Exit =>buf[0] = 5u8,
        }
    }

    fn from_bytes_flag(byte: &u8) -> anyhow::Result<Flag> {
        match byte {
            1 => Ok(Self::Ok),
            2 => Ok(Flag::Add),
            3 => Ok(Flag::List),
            4 => Ok(Flag::File),
            // 5 => Ok(Flag::Exit),
            _ => bail!("Flag inválida"),
        }
    }
}

pub fn b_take_while(bytes: &[u8], predicate: impl Fn(u8) -> bool) -> &[u8] { //Deprecated
    let mut idx = 0;
    for (i, byte) in bytes.iter().enumerate() {
        if !predicate(*byte) {
            break;
        }
        idx = i;
    }
    let (result, _) = bytes.split_at(idx + 1);
    result
}
