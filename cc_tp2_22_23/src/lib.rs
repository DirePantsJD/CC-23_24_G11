#![allow(dead_code)]

//TODO: Cenas de DNS
//TODO: Meta Dados
pub mod fstp {
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
        List,
        File,
    }

    impl<'a> FstpMessage<'a> {
        pub fn put_in_bytes(self, buf: &mut [u8]) -> anyhow::Result<()> {
            let flag = &self.header.flag;
            buf[0] = flag.to_bytes();
            let b_data_size: [u8; 2] = self.header.data_size.to_be_bytes();
            buf[1..3].copy_from_slice(&b_data_size);
            if let Some(data) = self.data {
                buf[3..3 + data.len()].copy_from_slice(data);
            }
            Ok(())
        }

        pub fn from_bytes(bytes: &[u8]) -> anyhow::Result<FstpMessage> {
            let (header, data) = bytes.split_at(3);
            let flag = Flag::from_bytes(&header[0])?;
            let b_data_size =
                u16::from_be_bytes(header[1..3].try_into().unwrap());
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
            }
        }

        fn from_bytes(byte: &u8) -> anyhow::Result<Self> {
            match byte {
                1 => Ok(Self::Ok),
                2 => Ok(Self::Add),
                3 => Ok(Self::List),
                4 => Ok(Self::File),
                _ => bail!("Flag inválida"),
            }
        }
    }
}

pub mod file_meta {
    use std::str::from_utf8;

    #[derive(Debug, Clone, Hash)]
    pub struct FileMeta {
        pub size: u64,
        pub n_blocks: u32,
        pub name: String,
    }

    impl FileMeta {
        pub fn as_bytes(self) -> Vec<u8> {
            let s_bs = self.name.as_bytes();
            let mut buf = Vec::with_capacity(12 + s_bs.len());
            let b_size = self.size.to_be_bytes();
            let b_n_blocks = self.size.to_be_bytes();
            buf[..8].copy_from_slice(&b_size);
            buf[8..12].copy_from_slice(&b_n_blocks);
            buf[12..12 + s_bs.len()].copy_from_slice(s_bs);
            buf
        }

        pub fn from_bytes(bytes: &[u8]) -> Self {
            let size = u64::from_be_bytes(bytes[0..8].try_into().unwrap());
            let n_blocks = u32::from_be_bytes(bytes[8..12].try_into().unwrap());
            let name = String::from(from_utf8(&bytes[12..]).unwrap());
            FileMeta {
                size,
                n_blocks,
                name,
            }
        }
    }
    impl PartialEq for FileMeta {
        fn eq(&self, other: &Self) -> bool {
            self.name == other.name
        }
    }
    impl Eq for FileMeta {}
}
