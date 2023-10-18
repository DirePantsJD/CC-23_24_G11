#![allow(dead_code)]

use anyhow::bail;

#[derive(Debug)]
pub struct FstpMessage<'a> {
    pub header: FstpHeader,
    pub data: &'a [u8],
}

#[derive(Debug)]
pub struct FstpHeader {
    pub val: Val,
}

#[derive(Debug)]
pub enum Val {
    Ok,
    Add,
    List,
    File,
    Start,
    End,
}

impl<'a> FstpMessage<'a> {
    pub fn to_bytes(self, buf: &mut [u8]) {
        let val = &self.header.val;
        val.to_bytes_val(buf);
    }
    pub fn from_bytes(bytes: &[u8]) -> anyhow::Result<FstpMessage> {
        let val = Val::from_bytes_val(bytes.first().expect("Empty message"))?;
        Ok(FstpMessage {
            header: FstpHeader { val },
            data: &[0u8],
        }) // return implicito
    }
}

impl Val {
    fn to_bytes_val(&self, buf: &mut [u8]) {
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

    fn from_bytes_val(byte: &u8) -> anyhow::Result<Val> {
        match byte {
            0 => Ok(Self::Ok),
            1 => Ok(Val::Add),
            2 => Ok(Val::List),
            3 => Ok(Val::File),
            4 => Ok(Val::Start),
            5 => Ok(Val::End),
            _ => bail!("Val inválido"),
        }
    }
}
