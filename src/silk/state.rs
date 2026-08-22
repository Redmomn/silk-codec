use std::ffi::c_void;
use std::ptr::NonNull;

use super::common::sdk_check;
use super::error::SilkError;
use super::sdk::bindings as sdk;

struct AlignedState {
    ptr: NonNull<c_void>,
    _size: usize,
}

impl AlignedState {
    fn new(size: i32) -> Result<Self, SilkError> {
        let size = usize::try_from(size).map_err(|_| SilkError::AllocationFailed)?;
        let ptr = NonNull::new(unsafe { libc::malloc(size) }).ok_or(SilkError::AllocationFailed)?;
        Ok(Self { ptr, _size: size })
    }

    fn as_ptr(&self) -> *mut c_void {
        self.ptr.as_ptr()
    }
}

impl Drop for AlignedState {
    fn drop(&mut self) {
        unsafe { libc::free(self.ptr.as_ptr()) };
    }
}

pub(crate) struct EncoderState(AlignedState);

impl EncoderState {
    pub(crate) fn new() -> Result<Self, SilkError> {
        let mut size = 0;
        sdk_check(unsafe { sdk::SKP_Silk_SDK_Get_Encoder_Size(&mut size) })?;
        Ok(Self(AlignedState::new(size)?))
    }

    pub(crate) fn as_ptr(&self) -> *mut c_void {
        self.0.as_ptr()
    }
}

pub(crate) struct DecoderState(AlignedState);

impl DecoderState {
    pub(crate) fn new() -> Result<Self, SilkError> {
        let mut size = 0;
        sdk_check(unsafe { sdk::SKP_Silk_SDK_Get_Decoder_Size(&mut size) })?;
        Ok(Self(AlignedState::new(size)?))
    }

    pub(crate) fn as_ptr(&self) -> *mut c_void {
        self.0.as_ptr()
    }
}
