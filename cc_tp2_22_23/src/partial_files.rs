pub mod partial_file {

    use anyhow::Result;
    use std::fs::File;
    use std::io::{Read, Seek, SeekFrom, Write};

    /// Writes the given block to the specified block index in the partial file.
    ///
    /// # Arguments
    ///
    /// * `file` - A mutable reference to the file to write to.
    /// * `block_index` - The index of the block to be written.
    /// * `block` - The block to be written into the file in the specified index.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the write was successful, otherwise returns an `anyhow::Error`.
    pub fn write_block(
        file: &mut File,
        block_index: u32,
        block: &[u8],
    ) -> Result<()> {
        file.seek(SeekFrom::Start(block_index.into()))?;
        file.write_all(block)?;
        Ok(())
    }

    /// Reads a block from a file at a given index.
    ///
    /// # Arguments
    ///
    /// * `file` - A mutable reference to the file to read from.
    /// * `block_index` - The index of the block to be read.
    /// * `block` - A mutable reference to a byte slice to store the read block.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the operation was successful,  otherwise returns an `anyhow::Error`.
    pub fn read_block(
        file: &mut File,
        block_index: u32,
        block: &mut [u8],
    ) -> Result<()> {
        file.seek(SeekFrom::Start(block_index.into()))?;
        file.read_exact(block)?;
        Ok(())
    }
}
