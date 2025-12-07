use std::ops::Deref;

use freerdp_sys::{GDI_RGN, HANDLE};
use shared::log;

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

pub fn normalize_rects(rects_raw: &[GDI_RGN], width: u32, height: u32) -> Option<Vec<Rect>> {
    let width = width as i32;
    let height = height as i32;
    rects_raw
        .iter()
        .filter_map(|r| {
            if r.x <= width
                && r.y < height
                && r.x >= 0
                && r.y >= 0
                && r.w <= width
                && r.w > 0
                && r.h <= height
                && r.h > 0
            {
                Some(Rect {
                    x: r.x as u32,
                    y: r.y as u32,
                    w: r.w as u32,
                    h: r.h as u32,
                })
            } else {
                log::debug!("Skipping invalid rect: {:?}", r);
                None
            }
        })
        .collect::<Vec<_>>()
        .into()

    // for rect in rects_raw {
    //     // If any rect is invalid, skip it
    //     if rect.x > width
    //         || rect.x < 0
    //         || rect.y > height
    //         || rect.y < 0
    //         || rect.w > width
    //         || rect.w < 0
    //         || rect.h > height
    //         || rect.h < 0
    //     {
    //         log::debug!("Skipping invalid rect: {:?}", rect);
    //         log::debug!("All rects: {:?}", rects_raw);
    //         return Some(vec![Rect{ x: 0, y: 0, w: width as u32, h: height as u32 }])
    //     }

    //     #[allow(clippy::unnecessary_cast)] // Maybe on other platforms is not an i32...
    //     rects.push(rect.into());
    // }
    // if rects.is_empty() { None } else { Some(rects) }
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
