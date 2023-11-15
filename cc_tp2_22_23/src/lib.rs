#![allow(dead_code)]
#![feature(ip_bits)]

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
    use std::hash::{Hash, Hasher};
    use std::io::{Read, Write};
    use std::str::from_utf8;

    #[derive(Debug, Clone)]
    pub struct FileMeta {
        pub f_size: u64,
        pub has_full_file: bool,
        pub blocks_len: u32,
        pub name_len: u16,
        pub blocks: BitVec<u8, Msb0>,
        pub name: String,
    }

    impl FileMeta {
        pub fn as_bytes(self, buf: &mut [u8]) -> anyhow::Result<usize> {
            let blocks_len = self.blocks_len as usize;
            let b_f_size = self.f_size.to_be_bytes();
            let has_ffile = self.has_full_file;
            let b_has_ff = if has_ffile { [1u8] } else { [0u8] };
            let b_blocks_len = self.blocks_len.to_be_bytes();
            let b_name_len = self.name_len.to_be_bytes();
            let mut b_blocks_buff = [0u8; 1000];
            self.blocks.clone().read(&mut b_blocks_buff)?;
            let b_name = self.name.as_bytes();

            buf[..8].copy_from_slice(&b_f_size);
            buf[8..9].copy_from_slice(&b_has_ff);
            buf[9..13].copy_from_slice(&b_blocks_len);
            buf[13..15].copy_from_slice(&b_name_len);
            buf[15..15 + blocks_len]
                .copy_from_slice(&b_blocks_buff[..blocks_len]);
            buf[15 + blocks_len..15 + blocks_len + b_name.len()]
                .copy_from_slice(b_name);
            Ok(15 + blocks_len as usize + b_name.len())
        }

        pub fn from_bytes(bytes: &[u8]) -> anyhow::Result<(usize, Self)> {
            let f_size = u64::from_be_bytes(bytes[0..8].try_into().unwrap());
            let has_full_file = if bytes[8] == 1 { true } else { false };
            let blocks_len =
                u32::from_be_bytes(bytes[9..13].try_into().unwrap());
            let name_len =
                u16::from_be_bytes(bytes[13..15].try_into().unwrap());
            let mut blocks = BitVec::<u8, Msb0>::new();
            blocks.write(&bytes[15..15 + blocks_len as usize])?;
            println!("bl:{},nl:{}", blocks_len, name_len);
            let name = String::from(
                from_utf8(
                    &bytes[15..15 + blocks_len as usize + name_len as usize],
                )
                .unwrap(),
            );
            let fm = FileMeta {
                f_size,
                has_full_file,
                blocks_len,
                name_len,
                blocks,
                name,
            };
            Ok((15 + blocks_len as usize + name_len as usize, fm))
        }
    }
    impl PartialEq for FileMeta {
        fn eq(&self, other: &Self) -> bool {
            self.name == other.name
        }
    }
    impl Eq for FileMeta {}

    impl Hash for FileMeta {
        fn hash<H: Hasher>(&self, state: &mut H) {
            self.name.hash(state);
        }
    }
}

pub mod peers_with_blocks {
    use std::collections::{HashMap, HashSet};
    use std::net::{IpAddr, Ipv4Addr};

    #[derive(Debug)]
    pub struct PeersWithFile {
        pub n_blocks: u32,
        pub peers_with_file: HashSet<IpAddr>,
        pub peers_with_blocks: HashMap<u32, HashSet<IpAddr>>,
    }

    impl PeersWithFile {
        pub fn new(n_blocks: u32) -> Self {
            PeersWithFile {
                n_blocks,
                peers_with_file: HashSet::new(),
                peers_with_blocks: HashMap::new(),
            }
        }

        pub fn to_bytes(self, buf: &mut [u8]) -> u16 {
            let mut offset = 0;
            let p_w_f_len = self.peers_with_file.len() as u16;
            let b_p_w_f_len = p_w_f_len.to_be_bytes();
            let mut p_w_f_buf = [0u8; 400];
            let p_w_f_buf_size =
                Self::bin_p_w_f(self.peers_with_file, &mut p_w_f_buf);

            let mut p_w_b_buf = [0u8; 1000];
            let p_w_b_buf_size = Self::bin_p_w_b(
                self.peers_with_blocks,
                &mut p_w_b_buf,
                self.n_blocks,
            );

            if p_w_f_len != 0 {
                buf[0..2].copy_from_slice(&b_p_w_f_len);
                buf[2..2 + p_w_f_buf_size]
                    .copy_from_slice(&p_w_f_buf[..p_w_f_buf_size]);
                offset += 2 + p_w_f_buf_size;
                buf[offset..offset + p_w_b_buf_size]
                    .copy_from_slice(&p_w_b_buf[..p_w_b_buf_size]);
                offset += p_w_b_buf_size
            }
            offset as u16
        }

        fn bin_p_w_f(p_w_f: HashSet<IpAddr>, buf: &mut [u8]) -> usize {
            let mut size = 0;
            for (i, ip) in p_w_f.iter().enumerate() {
                match ip {
                    IpAddr::V4(ipv4) => {
                        let b_ip = ipv4.to_bits().to_be_bytes();
                        let lower: usize = i * 4;
                        let upper = lower + 4;
                        buf[lower..upper].copy_from_slice(&b_ip);
                        size = upper;
                    }
                    _ => {}
                }
            }
            size
        }

        fn bin_p_w_b(
            p_w_b: HashMap<u32, HashSet<IpAddr>>,
            buf: &mut [u8],
            n_blocks: u32,
        ) -> usize {
            let mut offset = 0;
            for b_id in 0..n_blocks {
                if let Some(ips_set) = p_w_b.get(&b_id) {
                    buf[offset..offset + 4]
                        .copy_from_slice(&ips_set.len().to_be_bytes());
                    offset += 4;
                    for ip in ips_set {
                        match ip {
                            IpAddr::V4(ipv4) => {
                                let b_ip = ipv4.to_bits().to_be_bytes();
                                buf[offset..offset + 4].copy_from_slice(&b_ip);
                                offset += 4;
                            }
                            _ => {}
                        }
                    }
                } else {
                    buf[offset..offset + 4].copy_from_slice(&[0, 0, 0, 0]);
                    offset += 4;
                }
            }
            offset
        }

        pub fn from_bytes(bytes: &[u8]) -> anyhow::Result<PeersWithFile> {
            let n_ips_w_f = u16::from_be_bytes(bytes[0..2].try_into().unwrap());
            let mut offset: usize = 2;
            let mut peers_with_file = HashSet::<IpAddr>::new();
            let mut peers_with_blocks = HashMap::<u32, HashSet<IpAddr>>::new();
            let mut block_id: u32 = 0;
            for _ in 0..n_ips_w_f {
                let ip_bits = u32::from_be_bytes(
                    bytes[offset..offset + 4].try_into().unwrap(),
                );
                offset += 4;
                let ip = IpAddr::V4(Ipv4Addr::from_bits(ip_bits));
                peers_with_file.insert(ip);
            }

            while offset < bytes.len() {
                let n_ips_w_b = u32::from_be_bytes(
                    bytes[offset..offset + 4].try_into().unwrap(),
                );
                offset += 4;
                for _ in 0..n_ips_w_b {
                    let ip_bits = u32::from_be_bytes(
                        bytes[offset..offset + 4].try_into().unwrap(),
                    );
                    offset += 4;
                    if let None = peers_with_blocks.get(&block_id) {
                        peers_with_blocks.insert(block_id, HashSet::new());
                    }
                    let ip = IpAddr::V4(Ipv4Addr::from_bits(ip_bits));
                    peers_with_blocks.get_mut(&block_id).unwrap().insert(ip);
                }
                block_id += 1;
            }
            Ok(PeersWithFile {
                n_blocks: block_id,
                peers_with_file,
                peers_with_blocks,
            })
        }
    }
}
