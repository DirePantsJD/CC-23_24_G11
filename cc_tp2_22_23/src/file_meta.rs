use std::hash::{Hash, Hasher};
use std::io::Write;
use std::str::from_utf8;

#[derive(Debug, Clone)]
pub struct FileMeta {
    pub f_size: u64,
    pub has_full_file: bool,
    pub blocks_len: u32,
    pub name_len: u16,
    pub blocks: Vec<u8>,
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
        let b_blocks = self.blocks.as_slice();
        let b_name = self.name.as_bytes();
        dbg!(&b_blocks, &self.name);
        buf[..8].copy_from_slice(&b_f_size);
        buf[8..9].copy_from_slice(&b_has_ff);
        buf[9..13].copy_from_slice(&b_blocks_len);
        buf[13..15].copy_from_slice(&b_name_len);
        buf[15..15 + blocks_len].copy_from_slice(&b_blocks);
        buf[15 + blocks_len..15 + blocks_len + b_name.len()]
            .copy_from_slice(b_name);
        Ok(15 + blocks_len as usize + b_name.len())
    }

    pub fn from_bytes(bytes: &[u8]) -> anyhow::Result<(usize, Self)> {
        let f_size = u64::from_be_bytes(bytes[0..8].try_into().unwrap());
        let has_full_file = if bytes[8] == 1 { true } else { false };
        let blocks_len = u32::from_be_bytes(bytes[9..13].try_into().unwrap());
        let name_len = u16::from_be_bytes(bytes[13..15].try_into().unwrap());
        let mut blocks = Vec::<u8>::new();
        blocks.write(&bytes[15..15 + blocks_len as usize])?;
        println!("bl:{},nl:{}", blocks_len, name_len);
        let name = String::from(
            from_utf8(
                &bytes[15 + blocks_len as usize
                    ..15 + blocks_len as usize + name_len as usize],
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
