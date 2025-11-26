#![allow(warnings)]
#![allow(clippy::all)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

unsafe extern "C" {
    pub fn set_rust_get_access_token_cb(
        cb: extern "C" fn(
            instance: *mut freerdp,
            token_type: AccessTokenType,
            token: *mut *mut ::std::os::raw::c_char,
            count: usize,
            data: *const *const ::std::os::raw::c_char,
        ) -> BOOL,
    );

    pub fn get_access_token_wrapper(
        instance: *mut freerdp,
        token_type: AccessTokenType,
        token: *mut *mut ::std::os::raw::c_char,
        count: usize,
        ...
    ) -> BOOL;
}
