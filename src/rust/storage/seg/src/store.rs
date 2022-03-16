/// Copies `size` bytes at `byte_ptr` to the `offset` of `data`
/// Returns the next `offset`, that is, the next byte of `data` to be copied into
pub fn store_bytes_and_update_offset(
    byte_ptr: *const u8,
    offset: usize,
    size: usize,
    data: &mut [u8],
) -> usize {
    // get corresponding bytes from byte pointer
    let bytes = unsafe { ::std::slice::from_raw_parts(byte_ptr, size) };

    let end = offset + size;

    // store `bytes` to `data`
    data[offset..end].copy_from_slice(bytes);

    // next `offset`
    end
}
