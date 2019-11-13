// ccommon - a cache common library.
// Copyright (C) 2019 Twitter, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use ccommon::buf::OwnedBuf;
use pelikan::core::{DataProcessor, DataProcessorError};

pub struct PingDataProcessor;

impl DataProcessor for PingDataProcessor {
    type SockState = ();

    fn read(
        &mut self,
        rbuf: &mut OwnedBuf,
        wbuf: &mut OwnedBuf,
        _: &mut Option<&mut ()>,
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
        &mut self,
        rbuf: &mut OwnedBuf,
        wbuf: &mut OwnedBuf,
        _: &mut Option<&mut ()>,
    ) -> Result<(), DataProcessorError> {
        trace!("post-write processing");

        rbuf.reset();
        rbuf.shrink().expect("Failed to resize buffer");

        wbuf.reset();
        wbuf.shrink().expect("Failed to resize buffer");

        Ok(())
    }

    fn error(
        &mut self,
        rbuf: &mut OwnedBuf,
        wbuf: &mut OwnedBuf,
        _: &mut Option<&mut ()>,
    ) -> Result<(), DataProcessorError> {
        trace!("post-error processing");

        rbuf.reset();
        rbuf.shrink().expect("Failed to resize buffer");

        wbuf.reset();
        wbuf.shrink().expect("Failed to resize buffer");

        Ok(())
    }
}
