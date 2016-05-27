extern crate gst;
extern crate gtk;
extern crate gobject_sys;

use std::ffi::{CStr, CString};
use gst::ElementT;
use std::env;
use gtk::prelude::*;
use gobject_sys::{g_value_get_boxed, g_value_array_get_nth, g_value_get_double}; // TODO: use wrappers provided by gtk-rs and friends


// Helpers that should go into gstreamer1.0-rs

fn gst_structure_get_double(st: &gst::ffi::GstStructure, name: &str) -> f64 {
    unsafe {
        let gst_array_val = gst::ffi::gst_structure_get_value(st, CString::new(name).unwrap().as_ptr());
        let array_val = gst_array_val as *const gobject_sys::GValue;
        let arr = g_value_get_boxed(array_val) as *mut gobject_sys::GValueArray;
        let v = g_value_array_get_nth(arr, 0);
        g_value_get_double(v)
    }
}

fn gst_message_get_name(message: &gst::Message) -> Option<String>
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

fn gst_message_get_double(message: &gst::Message, key: &str) -> f64
{
    unsafe {
        let st = message.structure();
        gst_structure_get_double(&*st, key)
    }
}

// Level logic by itself

const SILENCE_AVG : i64 = 5;

struct Silence {
    // output
    silent: bool,

    // state changes per sample
    avg_rms: f64, // running average computation
    silent_period: i64, // time there has been silence

    // parameters (constant since construction)
    samples_to_become_silent: i64,
    become_silent_threshold: f64,
    become_active_threshold: f64, // hysteresis needs these two to be different

    // debug
    cycle: i64
}

impl Silence {
    fn new(silent_threshold: f64, active_threshold: f64, samples: i64) -> Silence {
        Silence {
            become_active_threshold: active_threshold,
            become_silent_threshold: silent_threshold,
            samples_to_become_silent: samples,
            silent: true,
            avg_rms: 0.0f64,
            silent_period: 0,

            // debug
            cycle: 0,
        }
    }
    fn input(&self, rms: f64) -> Silence {
        let silence_avg_f64 = SILENCE_AVG as f64;
        let silence_avg_minus_1_f64 = (SILENCE_AVG - 1) as f64;
        let avg_rms = rms / silence_avg_f64 + silence_avg_minus_1_f64 / silence_avg_f64 * self.avg_rms;
        let is_silence = match self.silent {
            true => self.avg_rms > self.become_active_threshold,
            false => self.avg_rms < self.become_silent_threshold
        };
        let silent_period = match is_silence {
            true => self.silent_period + 1,
            false => 0
        };
        let silent = match self.silent {
            true => is_silence,
            false => !is_silence && self.silent_period >= self.samples_to_become_silent
        };
        Silence {
            avg_rms: avg_rms,
            silent_period: silent_period,
            silent : silent,
            cycle : self.cycle + 1,
            .. *self
        }
    }
    fn output(&self) -> bool {
        self.silent
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

	mainloop.spawn();
	pipeline.play();

    let mut prev = true;
    let mut silence = Silence::new(-70f64, -65f64, 10);

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
                // level sends messages, look for rms, peak and decay doubles in the structure
                match gst_message_get_name(&message) {
                    Some(the_name) => {
                        match &*the_name {
                            "level" => {
                                let rms = gst_message_get_double(&message, "rms");
                                println!("{}: rms: {}", silence.cycle, rms);
                                silence = silence.input(rms);
                                let (silent_period, output) =
                                {
                                    let ref q_silence = silence;
                                    (q_silence.silent_period, q_silence.output())
                                };
                                match (output, output != prev) {
                                    (true, true) => println!("it became silent! output = {} prev = {}, {}", output, prev, rms),
                                    (false, true) => println!("it became active! {}", rms),
                                    (false, false) => println!("still active, {}, silent time of {}", rms, silent_period),
                                    (true, false) => println!("still silent"),
                                }
                                prev = output;
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