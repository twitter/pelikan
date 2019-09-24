use crate::protocol::Protocol;

use std::fmt;
use std::marker::PhantomData;

static mut ADMIN: Option<AdminInner> = None;

/// Handler for dealing with requests on the admin port.
pub trait AdminHandler {
    type Protocol: Protocol;

    fn process_request(
        &mut self,
        rsp: &mut <Self::Protocol as Protocol>::Response,
        req: &mut <Self::Protocol as Protocol>::Request,
    );
}

/// This is basically a manual vtable since there's no real way
/// to get this to work without using function pointers
struct AdminInner {
    data: *mut (),
    process_request: unsafe fn(data: *mut (), rsp: *mut (), req: *mut ()),
}

/// Manages the current global `AdminHandler` instance.
pub struct Admin<'a, H: 'a>
where
    H: AdminHandler,
{
    _marker: PhantomData<&'a H>,
}

impl<'a, H: 'a> Admin<'a, H>
where
    H: AdminHandler,
{
    pub unsafe fn new_global(handler: H) -> Result<Self, AdminCreationError> {
        if ADMIN.is_some() {
            return Err(AdminCreationError(()));
        }

        ADMIN = Some(AdminInner {
            data: Box::into_raw(Box::new(handler)) as *mut (),
            process_request: call_process_request::<H>,
        });

        Ok(Self {
            _marker: PhantomData,
        })
    }
}

impl<'a, H> Drop for Admin<'a, H>
where
    H: AdminHandler,
{
    fn drop(&mut self) {
        unsafe {
            let inner = ADMIN.take().unwrap();
            // Make sure to drop data
            let _ = Box::from_raw(inner.data as *mut H);
        }
    }
}

#[derive(Debug)]
pub struct AdminCreationError(());

impl std::error::Error for AdminCreationError {}
impl fmt::Display for AdminCreationError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "A global admin protocol instance is already active")
    }
}

unsafe fn call_process_request<H: AdminHandler>(data: *mut (), rsp: *mut (), req: *mut ()) {
    assert!(!data.is_null());
    assert!(!rsp.is_null());
    assert!(!req.is_null());

    let handler = &mut *(data as *mut H);
    let rsp = &mut *(rsp as *mut <H::Protocol as Protocol>::Response);
    let req = &mut *(req as *mut <H::Protocol as Protocol>::Request);

    handler.process_request(rsp, req);
}

#[no_mangle]
unsafe extern "C" fn admin_process_request(req: *mut (), rsp: *mut ()) {
    let admin = match ADMIN {
        Some(ref admin) => admin,
        None => {
            // TODO(sean): panic or error?
            error!("attempted to process request with no admin handler set up");
            return;
        }
    };

    (admin.process_request)(admin.data, req, rsp);
}
