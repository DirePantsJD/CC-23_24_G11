use anyhow::{bail, Ok, Result};
use std::fs;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};

/// Creates a partial file with the given `file_name` and `file_size`.
/// The file is created with the extension `.part`.
///
/// # Arguments
///
/// * `file_name` - A string slice that holds the name of the file to be created.
/// * `block_len` - An unsigned 32-bit integer that holds the number of files.
/// * `block_size` - An unsigned 32-bit integer that holds the size of file blocks.
///
/// # Returns
///
/// Returns a `Result` indicating whether the operation was successful or not.
pub fn create_part_file(
    file_name: &str,
    block_len: u32,
    block_size: u32,
) -> Result<()> {
    let file = File::create(format!("{}.part", file_name))?;
    file.set_len((block_len * (block_size + 1)).into())?;
    Ok(())
}

/// Completes a partial file by removing its metadata and the ".part" extension.
///
/// # Arguments
///
/// * `partial_file_name` - A string slice that holds the name of the partial file.
/// * `block_len` - An unsigned 32-bit integer that holds the number of files.
/// * `block_size` - An unsigned 32-bit integer that holds the size of file blocks.
///
/// # Returns
///
/// Returns a `Result` indicating whether the operation was successful or not.
pub fn complete_part_file(
    partial_file_name: &str,
    block_len: u32,
    block_size: u32,
) -> Result<()> {
    let mut file_name = partial_file_name.to_owned();

    if file_name.ends_with(".part") {
        let mut file = File::open(partial_file_name)?;
        let mut last_bytes = vec![0; block_len as usize];

        file.seek(SeekFrom::End(-(block_len as i64)))?;
        file.read_exact(&mut last_bytes)?;

        if last_bytes.iter().all(|b| *b == b'1') {
            // remove last block_len bytes (remove metadata)
            file.set_len((block_len * block_size).into())?;

            // remove .part extension
            file_name.truncate(file_name.len() - 5);
            fs::rename(partial_file_name, file_name.clone())?;
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
/// * `file` - A mutable reference to the file to write to.
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
    block_len: u32,
    block_size: u32,
    block_index: u32,
    block: &[u8],
) -> Result<()> {
    file.seek(SeekFrom::Start((block_index * block_size).into()))?;
    file.write_all(block)?;
    file.seek(SeekFrom::End((block_index - block_len).into()))?;
    file.write_all(&[b'1'])?;
    Ok(())
}

/// Reads a block from a partial file at a given block index.
///
/// # Arguments
///
/// * `file` - A mutable reference to the file to read from.
/// * `block_len` - An unsigned 32-bit integer that holds the number of files.
/// * `block_size` - The size of the blocks in the file.
/// * `block_index` - The index of the block to be read.
/// * `block` - A mutable reference to a byte slice to store the read block.
///
/// # Returns
///
/// Returns `Ok(())` if the operation was successful,  otherwise returns an `anyhow::Error`.
pub fn read_block_from_part_file(
    file: &mut File,
    block_len: u32,
    block_size: u32,
    block_index: u32,
    block: &mut [u8],
) -> Result<()> {
    file.seek(SeekFrom::End((block_index - block_len).into()))?;
    let mut buffer = [0; 1];
    file.read_exact(&mut buffer)?;
    if buffer[0] == b'1' {
        file.seek(SeekFrom::Start((block_index * block_size).into()))?;
        file.read_exact(block)?;
    } else {
        bail!("Block is not available");
    }
    Ok(())
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
    block_size: u32,
    block_index: u32,
    block: &mut [u8],
) -> Result<()> {
    file.seek(SeekFrom::Start((block_index * block_size).into()))?;
    file.read_exact(block)?;
    Ok(())
}
