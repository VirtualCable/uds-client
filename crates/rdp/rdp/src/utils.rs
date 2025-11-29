use std::ops::Deref;

use freerdp_sys::{GDI_RGN, HANDLE};

use super::geom::Rect;

pub fn pixel_format(bpp: u8, pixel_type: u8, a: u8, r: u8, g: u8, b: u8) -> u32 {
    ((bpp as u32) << 24)
        | ((pixel_type as u32) << 16)
        | ((a as u32) << 12)
        | ((r as u32) << 8)
        | ((g as u32) << 4)
        | (b as u32)
}

pub trait ToStringLossy {
    fn to_string_lossy(&self) -> String;
}

impl ToStringLossy for *const i8 {
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

pub fn normalize_invalids(rects_raw: &[GDI_RGN], width: u32, height: u32) -> Option<Vec<Rect>> {
    let mut rects = Vec::with_capacity(rects_raw.len());
    for rect in rects_raw {
        // If any rect is invalid, return full screen
        if rect.x > 0x3FFFFFFF
            || rect.x < 0
            || rect.y > 0x3FFFFFFF
            || rect.y < 0
            || rect.w > 0x3FFFFFFF
            || rect.w < 0
            || rect.h > 0x3FFFFFFF
            || rect.h < 0
        {
            return Some(vec![Rect::new(0, 0, width, height)]);
        }
        rects.push(Rect::new(
            rect.x as u32,
            rect.y as u32,
            rect.w as u32,
            rect.h as u32,
        ));
    }
    if rects.is_empty() { None } else { Some(rects) }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone)]
pub struct SafePtr<T> {
    ptr: std::ptr::NonNull<std::os::raw::c_void>,
    _marker: std::marker::PhantomData<T>,
}

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
