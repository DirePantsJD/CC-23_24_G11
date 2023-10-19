#![allow(dead_code)]

use anyhow::bail;

#[derive(Debug)]
pub struct FstpMessage<'a> {
    pub header: FstpHeader,
    pub data: &'a [u8],
}

#[derive(Debug)]
pub struct FstpHeader {
    pub flag: Flag,
}

#[derive(Debug)]
pub enum Flag {
    Ok,
    Add,
    List,
    File,
    Start,
    End,
}

impl<'a> FstpMessage<'a> {
    pub fn to_bytes(self, buf: &mut [u8]) {
        let flag = &self.header.flag;
        flag.to_bytes_flag(buf);
    }
    pub fn from_bytes(bytes: &[u8]) -> anyhow::Result<FstpMessage> {
        let flag =
            Flag::from_bytes_flag(bytes.first().expect("Empty message"))?;
        Ok(FstpMessage {
            header: FstpHeader { flag },
            data: &[0u8],
        }) // return implicito
    }
}

impl Flag {
    fn to_bytes_flag(&self, buf: &mut [u8]) {
        let mut i: u8 = 0;
        match self {
            Self::Ok => {}
            Self::Add => i = 1u8,
            Self::List => i = 2u8,
            Self::File => i = 3u8,
            Self::Start => i = 4u8,
            Self::End => i = 5u8,
        }
        buf[0] = i;
    }

    fn from_bytes_flag(byte: &u8) -> anyhow::Result<Flag> {
        match byte {
            0 => Ok(Self::Ok),
            1 => Ok(Flag::Add),
            2 => Ok(Flag::List),
            3 => Ok(Flag::File),
            4 => Ok(Flag::Start),
            5 => Ok(Flag::End),
            _ => bail!("Flag inválido"),
        }
    }
}
