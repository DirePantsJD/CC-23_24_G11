#![allow(dead_code)]

//TODO: Cenas de DNS
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
        pub fn as_bytes(self, buf: &mut [u8]) -> anyhow::Result<()> {
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
    use bitvec::prelude::*;
    use std::io::{Read, Write};
    use std::str::from_utf8;

    #[derive(Debug, Clone, Hash)]
    pub struct FileMeta {
        pub f_size: u64,
        pub has_full_file: bool,
        pub blocks_len: u32,
        pub name_len: u16,
        pub blocks: BitVec,
        pub name: String,
    }

    impl FileMeta {
        pub fn as_bytes(self, _buf: &mut [u8]) -> anyhow::Result<()> {
            let has_ffile = self.has_full_file;
            let s_bs = self.name.as_bytes();
            let mut buf = Vec::with_capacity(12 + s_bs.len());
            let b_f_size = self.f_size.to_be_bytes();
            let b_has_ff = if has_ffile { [1u8] } else { [0u8] };
            let b_blocks_len = self.blocks_len.to_be_bytes();
            let b_name_len = self.name_len.to_be_bytes();
            let mut b_blocks_buff = [0u8; 1000];
            self.blocks.clone().read(&mut b_blocks_buff)?;

            buf[..8].copy_from_slice(&b_f_size);
            buf[8..9].copy_from_slice(&b_has_ff);
            buf[9..13].copy_from_slice(&b_blocks_len);
            buf[13..15].copy_from_slice(&b_name_len);
            let blocks_len = self.blocks_len as usize;
            buf[15..15 + blocks_len]
                .copy_from_slice(&b_blocks_buff[..blocks_len]);
            buf[15 + blocks_len..15 + blocks_len + s_bs.len()]
                .copy_from_slice(s_bs);
            Ok(())
        }

        pub fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
            let f_size = u64::from_be_bytes(bytes[0..8].try_into().unwrap());
            let has_full_file = if bytes[8] == 1 { true } else { false };
            let blocks_len =
                u32::from_be_bytes(bytes[9..13].try_into().unwrap());
            let name_len =
                u16::from_be_bytes(bytes[13..15].try_into().unwrap());
            let mut blocks = BitVec::new();
            blocks.write(&bytes[15..15 + blocks_len as usize])?;
            let name = String::from(
                from_utf8(
                    &bytes[15..15 + blocks_len as usize + name_len as usize],
                )
                .unwrap(),
            );
            Ok(FileMeta {
                f_size,
                has_full_file,
                blocks_len,
                name_len,
                blocks,
                name,
            })
        }
    }
    impl PartialEq for FileMeta {
        fn eq(&self, other: &Self) -> bool {
            self.name == other.name
        }
    }
    impl Eq for FileMeta {}
}
