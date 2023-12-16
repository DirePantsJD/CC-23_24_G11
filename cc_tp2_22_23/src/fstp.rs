use anyhow::bail;
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
    AddBlock,
    List,
    File,
}

impl<'a> FstpMessage<'a> {
    pub fn as_bytes(self, buf: &mut [u8]) -> anyhow::Result<usize> {
        let flag = &self.header.flag;
        buf[0] = flag.to_bytes();
        let b_data_size: [u8; 2] = self.header.data_size.to_be_bytes();
        let data_size = u16::from_be_bytes(b_data_size).try_into().unwrap();
        buf[1..3].copy_from_slice(&b_data_size);
        if let Some(data) = self.data {
            buf[3..3 + data.len()].copy_from_slice(&data[..data_size]);
        }
        Ok(3 + self.data.unwrap_or(&[]).len())
    }

    pub fn from_bytes(bytes: &[u8]) -> anyhow::Result<FstpMessage> {
        let (header, data) = bytes.split_at(3);
        let flag = Flag::from_bytes(&header[0])?;
        let b_data_size = u16::from_be_bytes(header[1..3].try_into().unwrap());
        let data = if b_data_size == 0 {
            None
        } else {
            Some(&data[0..(b_data_size as usize)])
        };
        Ok(FstpMessage {
            header: FstpHeader {
                flag,
                data_size: b_data_size,
            },
            data,
        }) // return implicito
    }
}

impl Flag {
    fn to_bytes(&self) -> u8 {
        match self {
            Self::Ok => 1u8,
            Self::Add => 2u8,
            Self::List => 3u8,
            Self::File => 4u8,
            Self::AddBlock => 5u8,
        }
    }

    fn from_bytes(byte: &u8) -> anyhow::Result<Self> {
        match byte {
            1 => Ok(Self::Ok),
            2 => Ok(Self::Add),
            3 => Ok(Self::List),
            4 => Ok(Self::File),
            5 => Ok(Self::AddBlock),
            _ => bail!("Flag inválida"),
        }
    }
}
