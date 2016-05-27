extern crate gst;
extern crate gtk;
extern crate gobject_sys;

use std::ffi::{CStr, CString};
use gst::ElementT;
use std::env;
use gtk::prelude::*;
use gobject_sys::{g_value_get_boxed, g_value_array_get_nth, g_value_get_double}; // TODO: use wrappers provided by gtk-rs and friends


fn gst_structure_get_double(st: &gst::ffi::GstStructure, name: &str) -> f64 {
    unsafe {
        let gst_array_val = gst::ffi::gst_structure_get_value(st, CString::new(name).unwrap().as_ptr());
        let array_val = gst_array_val as *const gobject_sys::GValue;
        let arr = g_value_get_boxed(array_val) as *mut gobject_sys::GValueArray;
        let v = g_value_array_get_nth(arr, 0);
        g_value_get_double(v)
    }
}


fn main() {
    gst::init();
    let device = env::args().collect::<Vec<String>>()[1..].join(" ");
    let pipeline_str = format!("pulsesrc device={} ! level ! fakesink", device);
    let mut pipeline = gst::Pipeline::new_from_str(&pipeline_str).unwrap(); // format?
	let mut mainloop = gst::MainLoop::new();
	let mut bus = pipeline.bus().expect("Couldn't get bus from pipeline");
	let bus_receiver = bus.receiver();

    let mut silent = true;

	mainloop.spawn();
	pipeline.play();
	for message in bus_receiver.iter(){
		match message.parse(){
			gst::Message::StateChangedParsed{ref msg, ref old, ref new, ref pending} => {
				println!("element `{}` changed from {:?} to {:?}", message.src_name(), old, new);
			}
			gst::Message::ErrorParsed{ref msg, ref error, ref debug} => {
				println!("error msg from element `{}`: {}, quitting", message.src_name(), error.message());
				break;
			}
			gst::Message::Eos(ref msg) => {
				println!("eos received quiting");
				break;
			}
			_ => {
                let st_name_str;
                let rms;
                // level sends messages, look for rms, peak and decay doubles in the structure
                unsafe {
                    let st = message.structure();
                    if st.is_null() {
                        st_name_str = None;
                        rms = 0.0f64;
                    } else {
                        let st_name = gst::ffi::gst_structure_get_name(st);
                        st_name_str = Some(CStr::from_ptr(st_name).to_string_lossy().into_owned());
                        rms = gst_structure_get_double(&*st, "rms");
                    }
                }
                let peak = 0.0f64;
                let decay = 0.0f64;
                match st_name_str {
                    Some(the_name) => {
                        match &*the_name {
                            "level" => {
                                //println!("got level: rms = {}", rms);
                                if rms > -60f64 && silent {
                                    println!("not silent! {}", rms);
                                    silent = false;
                                } else if rms < -65f64 && !silent {
                                    println!("silent! {}", rms);
                                    silent = true;
                                }
                            }
                            _ => {
                                println!("got unknown structure name {}", the_name);
                            }
                        }
                    }
                    None => {
                        println!("msg of type `{}` from element `{}`", message.type_name(), message.src_name());
                    }
                }
			}
		}
	}
	mainloop.quit();
}
