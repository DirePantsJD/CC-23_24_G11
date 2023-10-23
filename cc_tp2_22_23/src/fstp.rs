#![allow(dead_code)]

use anyhow::bail;

#[derive(Debug)]
pub struct FstpMessage<'a> {
    pub header: FstpHeader,
    pub data: Option<&'a [u8]>,
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
    // Exit,
}

impl<'a> FstpMessage<'a> {
    pub fn put_in_bytes(self, buf: &mut Vec<u8>) -> anyhow::Result<()> {
        let flag = &self.header.flag;
        flag.to_bytes_flag(buf);
        if let Some(data) = self.data {
            buf.extend_from_slice(&data);
        }
        Ok(())
    }

    pub fn from_bytes(bytes: &[u8]) -> anyhow::Result<FstpMessage> {
        let (first, rest) = bytes.split_first().expect("Empty message");
        let flag = Flag::from_bytes_flag(first)?;
        let data = if rest.is_empty() {
            None
        } else {
            Some(b_take_while(rest, |b| b != 0))
        };
        Ok(FstpMessage {
            header: FstpHeader { flag },
            data,
        }) // return implicito
    }
}

impl Flag {
    fn to_bytes_flag(&self, buf: &mut Vec<u8>) {
        let mut i: u8 = 0;
        match self {
            Self::Add => i = 1u8,
            Self::List => i = 2u8,
            Self::File => i = 3u8,
            // Self::Exit => i = 4u8,
            Self::Ok => {}
        }
        buf.push(i);
    }

    fn from_bytes_flag(byte: &u8) -> anyhow::Result<Flag> {
        match byte {
            0 => Ok(Self::Ok),
            1 => Ok(Flag::Add),
            2 => Ok(Flag::List),
            3 => Ok(Flag::File),
            // 4 => Ok(Flag::Exit),
            _ => bail!("Flag inválida"),
        }
    }
}

pub fn b_take_while(bytes: &[u8], predicate: impl Fn(u8) -> bool) -> &[u8] {
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
