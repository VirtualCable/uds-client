// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
// All rights reserved.

use freerdp_sys::HANDLE;
use std::ops::Deref;

pub trait ToStringLossy {
    fn to_string_lossy(&self) -> String;
}

impl ToStringLossy for *const ::std::os::raw::c_char {
    fn to_string_lossy(&self) -> String {
        if self.is_null() {
            return String::new();
        }
        unsafe {
            std::ffi::CStr::from_ptr(*self)
                .to_string_lossy()
                .into_owned()
        }
    }
}

#[repr(transparent)]
#[derive(Debug)]
pub struct SafePtr<T> {
    ptr: std::ptr::NonNull<std::os::raw::c_void>,
    _marker: std::marker::PhantomData<T>,
}

impl<T> Clone for SafePtr<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for SafePtr<T> {}

unsafe impl<T> Send for SafePtr<T> {}
unsafe impl<T> Sync for SafePtr<T> {}

impl<T> SafePtr<T> {
    pub fn new(ptr: *mut T) -> Option<Self> {
        std::ptr::NonNull::new(ptr as *mut std::os::raw::c_void).map(|nn| Self {
            ptr: nn,
            _marker: std::marker::PhantomData,
        })
    }

    pub fn as_ptr(&self) -> *const T {
        self.ptr.as_ptr() as *const T
    }

    pub fn as_mut_ptr(&self) -> *mut T {
        self.ptr.as_ptr() as *mut T
    }
}

impl<T> Deref for SafePtr<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*(self.as_ptr()) }
    }
}

pub type SafeHandle = SafePtr<std::os::raw::c_void>;

impl SafeHandle {
    pub fn as_handle(&self) -> HANDLE {
        self.ptr.as_ptr() as HANDLE
    }
}
