extern crate gobject_sys;
extern crate gst;

use std::ffi::{CStr, CString};

use gobject_sys::{g_value_get_boxed, g_value_array_get_nth, g_value_get_double}; // TODO: use wrappers provided by gtk-rs and friends


// Helpers that should go into gstreamer1.0-rs


pub fn gst_structure_get_double(st: &gst::ffi::GstStructure, name: &str) -> f64 {
    unsafe {
        let gst_array_val = gst::ffi::gst_structure_get_value(st, CString::new(name).unwrap().as_ptr());
        let array_val = gst_array_val as *const gobject_sys::GValue;
        let arr = g_value_get_boxed(array_val) as *mut gobject_sys::GValueArray;
        let v = g_value_array_get_nth(arr, 0);
        g_value_get_double(v)
    }
}


pub fn gst_message_get_name(message: &gst::Message) -> Option<String>
{
    unsafe {
        let st = message.structure();
        if st.is_null() {
            None
        } else {
            let st_name = gst::ffi::gst_structure_get_name(st);
            Some(CStr::from_ptr(st_name).to_string_lossy().into_owned())
        }
    }
}


pub fn gst_message_get_double(message: &gst::Message, key: &str) -> f64
{
    unsafe {
        let st = message.structure();
        gst_structure_get_double(&*st, key)
    }
}
