use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, Ipv4Addr};

use crate::fsnp::MAX_CHUNK_SIZE;

#[derive(Debug)]
pub struct PeersWithFile {
    pub file_size: u32,
    pub n_blocks: u32,
    pub peers_with_file: HashSet<IpAddr>,
    pub peers_with_blocks: HashMap<u32, HashSet<IpAddr>>,
}

impl PeersWithFile {
    pub fn new(file_size: u32, n_blocks: u32) -> Self {
        PeersWithFile {
            file_size,
            n_blocks,
            peers_with_file: HashSet::new(),
            peers_with_blocks: HashMap::new(),
        }
    }

    pub fn to_bytes(self, buf: &mut [u8]) -> u16 {
        let mut offset = 0;
        let f_size_bytes = self.file_size.to_le_bytes();
        let p_w_f_len = self.peers_with_file.len() as u16;
        let b_p_w_f_len = p_w_f_len.to_be_bytes();
        let mut p_w_f_buf = [0u8; 400];
        let p_w_f_buf_size =
            Self::bin_p_w_f(self.peers_with_file, &mut p_w_f_buf);

        let mut p_w_b_buf = [0u8; 1000];

        let n_blocks =
            (self.file_size as f64 / MAX_CHUNK_SIZE as f64).ceil() as u32;
        let p_w_b_buf_size =
            Self::bin_p_w_b(self.peers_with_blocks, &mut p_w_b_buf, n_blocks);
        dbg!(&p_w_b_buf[..p_w_b_buf_size]);

        if p_w_f_len != 0 {
            buf[0..4].copy_from_slice(&f_size_bytes);
            buf[4..6].copy_from_slice(&b_p_w_f_len);
            buf[6..6 + p_w_f_buf_size]
                .copy_from_slice(&p_w_f_buf[..p_w_f_buf_size]);
            offset += 6 + p_w_f_buf_size;
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
        dbg!(&p_w_b, &n_blocks);
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
        let file_size = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
        let n_ips_w_f = u16::from_be_bytes(bytes[4..6].try_into().unwrap());
        let mut offset: usize = 6;
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
            file_size,
            n_blocks: block_id,
            peers_with_file,
            peers_with_blocks,
        })
    }
}
