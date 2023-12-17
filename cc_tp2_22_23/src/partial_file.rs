use anyhow::{bail, Context, Ok, Result};
use bitvec::order::Msb0;
use bitvec::vec::BitVec;
use std::fs::File;
use std::fs::{self, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::mem::size_of;
use std::path::PathBuf;

use crate::file_meta::FileMeta;
use crate::fsnp::MAX_CHUNK_SIZE;

/// Creates a partial file with the given `file_name` and `file_size`.
/// The file is created with the extension `.part`.
///
/// # Arguments
///
/// * `file_name` - A string slice that holds the name of the file to be created.
/// * `file_size` - An unsigned 32-bit integer that holds the size of the file.
/// * `n_blocks` - An unsigned 32-bit integer that holds the number of blocks in a file.
///
/// # Returns
///
/// Returns a `Result` indicating whether the operation was successful or not.
pub fn create_part_file(
    file_name: &str,
    file_size: u32,
    n_blocks: u32,
) -> Result<File> {
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(format!(".{}.part", file_name))?;
    file.set_len(
        (file_size
            + n_blocks
            + size_of::<u16>() as u32 //size of last block
            + size_of::<u32>() as u32) // n_blocks
            .into(),
    )?;
    Ok(file)
}

/// Completes a partial file by removing its metadata and the ".part" extension.
///
/// # Arguments
///
/// * `partial_file_name` - A string slice that holds the name of the partial file.
/// * `file_size` - An unsigned 32-bit integer that holds the size of the file.
/// * `block_len` - An unsigned 32-bit integer that holds the number of files.
///
/// # Returns
///
/// Returns a `Result` indicating whether the operation was successful or not.
pub fn complete_part_file(
    partial_file_name: &str,
    file_size: u32,
    n_blocks: u32,
) -> anyhow::Result<()> {
    let mut file_name = partial_file_name.to_owned();
    dbg & (file_name);
    if file_name.ends_with(".part") {
        let mut file =
            File::open(partial_file_name).context("Failed to open file")?;
        let mut meta_bytes = vec![0; n_blocks as usize];

        file.seek(SeekFrom::End(
            -(n_blocks as i64
                + size_of::<u16>() as i64
                + size_of::<u32>() as i64),
        ))?;
        file.read_exact(&mut meta_bytes)?;

        // if all chunks were filled
        dbg!(&meta_bytes);
        if meta_bytes.iter().all(|b| *b == b'1') {
            // remove file metadata
            file.set_len(file_size.into())?;

            // remove .part extension
            file_name.truncate(file_name.len() - 5);
            fs::rename(partial_file_name, &file_name[1..])
                .context("Invalid file rename")?;
        } else {
            bail!("File is not complete");
        }
    } else {
        bail!("File is not a partial file");
    }
    Ok(())
}

/// Writes the given block to the specified block index in the partial file.
///
/// # Arguments
///
/// * `file_path` - A reference to a str with the path of the file to write to.
/// * `block_len` - An unsigned 32-bit integer that holds the number of files.
/// * `block_size` - The size of the blocks in the file.
/// * `block_index` - The index of the block to be written.
/// * `block` - The block to be written into the file in the specified index.
///
/// # Returns
///
/// Returns `Ok(())` if the write was successful, otherwise returns an `anyhow::Error`.
pub fn write_block(
    file: &mut File,
    n_blocks: u32,
    block_size: u32,
    block_index: u32,
    block: &[u8],
) -> Result<()> {
    // write chunk
    file.seek(SeekFrom::Start(
        (block_index * MAX_CHUNK_SIZE as u32).into(),
    ))?;
    file.write_all(&block[..block_size as usize])?;

    // mark chunk as written in file metadata
    file.seek(SeekFrom::End(
        block_index as i64
            - n_blocks as i64
            - size_of::<u16>() as i64
            - size_of::<u32>() as i64
            - 1,
    ))?;
    file.write_all(&[b'1'])?;
    Ok(())
}

/// Reads a block from a partial file at a given block index.
///
/// # Arguments
///
/// * `file` - A mutable reference to the file to read from.
/// * `nblock_len` - An unsigned 32-bit integer that holds the number of block.
/// * `block_size` - The size of the blocks in the file.
/// * `block_index` - The index of the block to be read.
/// * `block` - A mutable reference to a byte slice to store the read block.
///
/// # Returns
///
/// Returns `Ok(())` if the operation was successful,  otherwise returns an `anyhow::Error`.
pub fn read_block_from_part_file(
    file: &mut File,
    _n_blocks: u32,
    block_size: u32,
    block_index: u32,
    block: &mut [u8],
) -> Result<usize> {
    // verify, in file metadata, if block was already written
    // file.seek(SeekFrom::End(
    //     (block_index
    //         - n_blocks
    //         - size_of::<u16>() as u32
    //         - size_of::<u32>() as u32)
    //         .into(),
    // ))?;
    // let mut buffer = [0; 1];
    // file.read_exact(&mut buffer)?;

    // if buffer[0] == b'1' {
    // write block
    // Se for ultimo
    file.seek(SeekFrom::Start(
        (block_index * MAX_CHUNK_SIZE as u32).into(),
    ))?;
    let n = file.read(&mut block[..block_size as usize])?;
    Ok(n)
    // } else {
    // bail!("Block is not available");
    // }
}

/// Reads a block from a complete file at a given index.
///
/// # Arguments
///
/// * `file` - A mutable reference to the file to read from.
/// * `block_size` - The size of the blocks in the file.
/// * `block_index` - The index of the block to be read.
/// * `block` - A mutable reference to a byte slice to store the read block.
///
/// # Returns
///
/// Returns `Ok(())` if the operation was successful,  otherwise returns an `anyhow::Error`.
pub fn read_block_from_complete_file(
    file: &mut File,
    _block_size: u32,
    block_index: u32,
    block: &mut [u8],
) -> Result<usize> {
    file.seek(SeekFrom::Start(
        (block_index * MAX_CHUNK_SIZE as u32).into(),
    ))?;
    let n = file.read(block)?;
    Ok(n)
}

/// Retrieves the metadata of a file specified by the given path.
///
/// # Arguments
///
/// * `path` - A `PathBuf` representing the path to the file.
///
/// # Returns
///
/// Returns a `Result` containing the `FileMeta` struct if successful, or an error if the metadata retrieval fails.
///
/// # Errors
///
/// This function may return an error if:
///
/// * The file metadata retrieval fails.
/// * The file cannot be opened.
/// * The file cannot be read.
///
/// # Panics
///
/// This function may panic if:
///
/// * The file name cannot be retrieved.
/// * The file extension is not "part".
/// * The file seek operation fails.
/// * The file read operation fails.
///
/// # Safety
///
/// This function assumes that the file is a valid file and that the path is a valid path to the file.
/// It also assumes that the file contains the expected data structure.
///
/// # Notes
///
/// This function is specifically designed to handle files with the extension "part".
/// It retrieves the file metadata, including the file size, whether the file is complete or partial,
/// the length of the blocks, the length of the file name, the bit vector representing the blocks,
/// and the file name itself.
/// If the file extension is not "part", it assumes that the file is complete and sets the appropriate values.
///
/// The `FileMeta` struct is defined as follows:
///
/// ```rust
/// pub struct FileMeta {
///     f_size: u64,
///     has_full_file: bool,
///     blocks_len: u32,
///     name_len: u16,
///     blocks: BitVec<u8, Msb0>,
///     name: String,
/// }
/// ```
///
/// The `BitVec` type is from the `bit-vec` crate and represents a vector of bits.
/// The `Msb0` type parameter specifies that the most significant bit is stored at index 0.
///
/// # See Also
///
/// * [`std::path::PathBuf`](https://doc.rust-lang.org/std/path/struct.PathBuf.html)
/// * [`std::fs::metadata`](https://doc.rust-lang.org/std/fs/fn.metadata.html)
/// * [`std::io::Result`](https://doc.rust-lang.org/std/io/type.Result.html)
/// * [`bit-vec`](https://crates.io/crates/bit-vec)
/// * [`BitVec`](https://docs.rs/bit-vec/0.6.3/bit_vec/struct.BitVec.html)
/// * [`Msb0`](https://docs.rs/bit-vec/0.6.3/bit_vec/struct.Msb0.html)
/// ```
pub fn get_file_metadata(path: &PathBuf) -> Result<FileMeta> {
    let meta = path.metadata().expect("Failed to get file metadata");
    let name = path
        .file_name()
        .and_then(|os_str| os_str.to_str())
        .expect("Failed to get file name");

    if path.extension().unwrap() == "part" {
        let mut file = File::open(path)?;

        // get bit vector size
        file.seek(SeekFrom::End(-(size_of::<u32>() as i64)))?;
        let mut block_len = [0; size_of::<u32>()];
        file.read_exact(&mut block_len)?;
        let block_len = u32::from_le_bytes(block_len);

        // get size of last chunk
        file.seek(SeekFrom::End(
            -((size_of::<u16>() + size_of::<u32>()) as i64),
        ))?;
        let mut last_block_size = [0; size_of::<u32>()];
        file.read_exact(&mut last_block_size)?;
        // let last_block_size = u32::from_le_bytes(last_block_size);

        // get bit vector
        file.seek(SeekFrom::End(
            -(block_len as i64
                + ((size_of::<u16>() + size_of::<u32>()) as i64)),
        ))?;
        let mut bit_vec = vec![0; block_len as usize];
        file.read_exact(&mut bit_vec)?;
        let mut blocks = BitVec::new();
        for byte in bit_vec.iter_mut() {
            if byte == &b'1' {
                blocks.push(true);
            } else {
                blocks.push(false);
            }
        }

        Ok(FileMeta {
            f_size: meta.len() - (block_len as u64 + size_of::<u32>() as u64),
            has_full_file: false,
            blocks_len: block_len,
            name_len: name.len() as u16,
            blocks,
            name: name.to_string(),
        })
    } else {
        Ok(FileMeta {
            f_size: meta.len(),
            has_full_file: true,
            blocks_len: 0,
            name_len: name.len() as u16,
            blocks: BitVec::<u8, Msb0>::new(),
            name: name.to_string(),
        })
    }
}

pub fn read_file(
    file_name: &str,
    block_index: u32,
    block_buf: &mut [u8],
) -> anyhow::Result<usize> {
    dbg!(&file_name);
    let mut file = File::open(file_name).unwrap();
    let file_size = file.metadata().unwrap().len();
    let n_blocks = (file_size as f64 / MAX_CHUNK_SIZE as f64).ceil() as u32;
    let block_size = if block_index == n_blocks - 1 {
        file_size as u32 % n_blocks as u32
    } else {
        MAX_CHUNK_SIZE as u32
    };
    dbg!(&file_size, &block_size, &n_blocks);

    if file_name.ends_with(".part") {
        read_block_from_part_file(
            &mut file,
            n_blocks,
            block_size,
            block_index,
            block_buf,
        )
    } else {
        read_block_from_complete_file(
            &mut file,
            block_size,
            block_index,
            block_buf,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // #[test]
    // fn create_pfile_t() {
    //     create_part_file("test_pfile", 1000, 100).unwrap(); //Works
    // }

    #[test]
    fn create_write_read_test() {
        let mut file = create_part_file("test_pfile", 1000, 100).unwrap(); //Works
        let block99 = b"Working???";
        let block00 = b"Is this...";
        let block49 = b"even rly..";
        write_block(&mut file, 100, 10, 99, block99).unwrap();
        write_block(&mut file, 100, 10, 0, block00).unwrap();
        write_block(&mut file, 100, 10, 49, block49).unwrap();
        let read_buf = &mut [0; 10];
        let n_read =
            read_block_from_complete_file(&mut file, 10, 99, &mut read_buf[..])
                .unwrap();
        dbg!(n_read, String::from_utf8(read_buf.to_vec()).unwrap());
    }
}
