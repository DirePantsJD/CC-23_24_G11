use anyhow::{Ok, Result};
use std::fs::{rename, File};
use std::io::{Read, Seek, SeekFrom, Write};

/// Creates a partial file with the given `file_name` and `file_size`.
/// The file is created with the extension `.part`.
///
/// # Arguments
///
/// * `file_name` - A string slice that holds the name of the file to be created.
/// * `file_size` - An unsigned 64-bit integer that holds the size of the file to be created.
///
/// # Returns
///
/// Returns a `Result` indicating whether the operation was successful or not.
pub fn create_part_file(file_name: &str, file_size: u64) -> Result<()> {
    let file = File::create(format!("{}.part", file_name))?;
    file.set_len(file_size)?;
    Ok(())
}

/// Renames a completed file by removing the ".part" extension from its name.
///
/// # Arguments
///
/// * `partial_file_name` - A string slice that holds the name of the partial file.
///
/// # Returns
///
/// Returns a `Result` indicating whether the operation was successful or not.
pub fn complete_file(partial_file_name: &str) -> Result<()> {
    let mut file_name = partial_file_name.to_owned();
    if file_name.ends_with(".part") {
        file_name.truncate(file_name.len() - 5);
        rename(partial_file_name, file_name)?;
    }
    Ok(())
}

/// Writes the given block to the specified block index in the partial file.
///
/// # Arguments
///
/// * `file` - A mutable reference to the file to write to.
/// * `block_index` - The index of the block to be written.
/// * `file_block_size` - The size of the blocks in the file.
/// * `block` - The block to be written into the file in the specified index.
///
/// # Returns
///
/// Returns `Ok(())` if the write was successful, otherwise returns an `anyhow::Error`.
pub fn write_block(
    file: &mut File,
    block_index: u32,
    file_block_size: u32,
    block: &[u8],
) -> Result<()> {
    file.seek(SeekFrom::Start((block_index * file_block_size).into()))?;
    file.write_all(block)?;
    Ok(())
}

/// Reads a block from a file at a given index.
///
/// # Arguments
///
/// * `file` - A mutable reference to the file to read from.
/// * `block_index` - The index of the block to be read.
/// * `file_block_size` - The size of the blocks in the file.
/// * `block` - A mutable reference to a byte slice to store the read block.
///
/// # Returns
///
/// Returns `Ok(())` if the operation was successful,  otherwise returns an `anyhow::Error`.
pub fn read_block(
    file: &mut File,
    block_index: u32,
    file_block_size: u32,
    block: &mut [u8],
) -> Result<()> {
    file.seek(SeekFrom::Start((block_index * file_block_size).into()))?;
    file.read_exact(block)?;
    Ok(())
}
