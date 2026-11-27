use super::{ProtocolError, Result, Role, Settings, Status};
use libz_rs_sys as zlib;

/// Raw inflater with access to zlib's sync-point and history-preserving reset.
pub(super) struct ConfiguredDecoder {
    stream: zlib::z_stream,
}

// SAFETY: The decoder exclusively owns its zlib-rs allocation and uses the
// process allocator. Input and output slices are borrowed only inside decompress.
unsafe impl Send for ConfiguredDecoder {}
unsafe impl Sync for ConfiguredDecoder {}

impl std::fmt::Debug for ConfiguredDecoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfiguredDecoder").finish_non_exhaustive()
    }
}

impl ConfiguredDecoder {
    pub(super) fn new(role: Role, settings: Settings) -> Self {
        let peer_window = match role {
            Role::Client => settings.server_max_window_bits,
            Role::Server => settings.client_max_window_bits,
        };
        let mut stream = zlib::z_stream::default();
        // SAFETY: The default stream supplies a matching allocator pair. The
        // version and size come from the same library that initializes it.
        let status = unsafe {
            zlib::inflateInit2_(
                &mut stream,
                -i32::from(peer_window),
                zlib::zlibVersion(),
                std::mem::size_of::<zlib::z_stream>() as i32,
            )
        };
        assert_eq!(status, zlib::Z_OK);
        Self { stream }
    }

    pub(super) fn decompress(
        &mut self,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<(Status, usize, usize)> {
        let input_len = input.len().min(u32::MAX as usize) as u32;
        let output_len = u32::try_from(output.len()).expect("bounded inflate scratch buffer");
        self.stream.next_in = input.as_ptr();
        self.stream.avail_in = input_len;
        self.stream.next_out = output.as_mut_ptr();
        self.stream.avail_out = output_len;
        // SAFETY: Both buffers cover the indicated lengths and live through this
        // call. Clear their pointers before either borrow can end.
        let status = unsafe { zlib::inflate(&mut self.stream, zlib::Z_NO_FLUSH) };
        let consumed = input_len - self.stream.avail_in;
        let produced = output_len - self.stream.avail_out;
        self.stream.next_in = std::ptr::null();
        self.stream.next_out = std::ptr::null_mut();
        self.stream.avail_in = 0;
        self.stream.avail_out = 0;
        let status = match status {
            zlib::Z_OK => Status::Ok,
            zlib::Z_BUF_ERROR => Status::BufError,
            zlib::Z_STREAM_END => {
                // RFC 7692 section 7.2.1 permits BFINAL before another block.
                // SAFETY: The stream is initialized and exclusively borrowed.
                let reset = unsafe { zlib::inflateResetKeep(&mut self.stream) };
                if reset != zlib::Z_OK {
                    return Err(ProtocolError::Compression.into());
                }
                Status::StreamEnd
            }
            _ => return Err(ProtocolError::Compression.into()),
        };
        Ok((status, consumed as usize, produced as usize))
    }

    pub(super) fn at_sync_point(&mut self) -> bool {
        // SAFETY: The stream is initialized and exclusively borrowed.
        unsafe { zlib::inflateSyncPoint(&mut self.stream) == 1 }
    }

    pub(super) fn reset(&mut self) {
        // SAFETY: The stream is initialized and exclusively borrowed. inflateReset
        // clears the window contents while preserving its negotiated width.
        assert_eq!(unsafe { zlib::inflateReset(&mut self.stream) }, zlib::Z_OK);
    }
}

impl Drop for ConfiguredDecoder {
    fn drop(&mut self) {
        // SAFETY: Each successful initialization has exactly one owner.
        unsafe { zlib::inflateEnd(&mut self.stream) };
    }
}
