extern crate libc;

use std::ffi::CString;
use libc::{c_int, c_char};

#[link(name = "android-properties")]
extern {
    fn property_get(key: *const c_char, value: *mut c_char, default_value: *const c_char) -> c_int;
    fn property_set(key: *const c_char, value: *const c_char) -> c_int;
}

pub fn set(key: &'static str, value: &'static str) -> bool {
    let c_key = CString::new(key).unwrap();
    let c_value = CString::new(value).unwrap();

    unsafe {
        let c_ret = property_set(c_key.as_ptr(), c_value.as_ptr());
        if c_ret == 1 {true} else {false}
    }
}

pub fn get(key: &'static str, default: &'static str) -> String {
    let c_key = CString::new(key).unwrap();
    let c_default = CString::new(default).unwrap();
    let c_value = CString::new("").unwrap();
    let raw_value = c_value.into_raw();

    unsafe {
        property_get(c_key.as_ptr(), raw_value, c_default.as_ptr());
        let _c_value = CString::from_raw(raw_value);

        _c_value.into_string().unwrap()
    }
}
