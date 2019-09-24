use ccommon::buf::Buf;
use pelikan::core::{DataProcessor, DataProcessorError};
use std::mem::MaybeUninit;

pub enum PingDataProcessor {}

impl DataProcessor for PingDataProcessor {
    type SockState = ();

    fn read(
        rbuf: &mut Buf,
        wbuf: &mut Buf,
        _: &mut *mut MaybeUninit<Self::SockState>,
    ) -> Result<(), DataProcessorError> {
        use pelikan_sys::protocol::ping::*;

        info!("post-read processing");

        unsafe {
            // keep parse-process-compose until we run out of data in rbuf
            while rbuf.read_size() > 0 {
                info!("{} bytes left", rbuf.read_size());

                let status = parse_req(rbuf.into_raw_mut());
                if status == PARSE_EUNFIN {
                    return Ok(());
                }
                if status != PARSE_OK {
                    return Err(DataProcessorError::Error);
                }

                if compose_rsp(&mut wbuf.into_raw_mut() as *mut _) != COMPOSE_OK {
                    return Err(DataProcessorError::Error);
                }
            }
        }

        Ok(())
    }

    fn write(
        _rbuf: &mut Buf,
        _wbuf: &mut Buf,
        _: &mut *mut MaybeUninit<Self::SockState>,
    ) -> Result<(), DataProcessorError> {
        trace!("post-write processing");

        // TODO(sean): The C implementation calls dbuf_shrink here

        Ok(())
    }

    fn error(
        rbuf: &mut Buf,
        wbuf: &mut Buf,
        _: &mut *mut MaybeUninit<Self::SockState>,
    ) -> Result<(), DataProcessorError> {
        trace!("post-error processing");

        // TODO(sean): The C implementation calls dbuf_shrink here
        rbuf.reset();
        wbuf.reset();

        Ok(())
    }
}
