extern crate argparse;
extern crate gst;
extern crate gtk;
extern crate gobject_sys;
extern crate itertools;

use std::process::Command;
use std::ffi::{CStr, CString};
use std::env;
use std::thread;

use itertools::Zip;
use argparse::ArgumentParser;
use gst::ElementT;
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


struct Silence {
    // output
    silent: bool,

    // state changes per sample
    avg_rms: f64, // running average computation
    silent_current: i64, // time there has been silence

    // parameters (constant since construction)
    silent_period: i64,
    become_silent_threshold: f64,
    become_active_threshold: f64, // hysteresis needs these two to be different
    average_period: i64,

    // debug
    cycle: i64
}


impl Silence {

    fn new(silent_threshold: f64, active_threshold: f64, silent_period: i64, average_period: i64) -> Silence {
        Silence {
            become_active_threshold: active_threshold,
            become_silent_threshold: silent_threshold,
            silent_period: silent_period,
            average_period: average_period,
            silent: true,
            avg_rms: silent_threshold,
            silent_current: 0,

            // debug
            cycle: 0,
        }
    }

    fn input(&self, rms: f64) -> Silence {
        //println!("pre silent: cycle {} avg_rms {} silent {} silent_current {} ( S->A {}   A->S {})", self.cycle, self.avg_rms, self.silent, self.silent_current, self.become_active_threshold, self.become_silent_threshold);
        let silence_avg_f64 = self.average_period as f64;
        let silence_avg_minus_1_f64 = (self.average_period - 1) as f64;
        let avg_rms = rms / silence_avg_f64 + silence_avg_minus_1_f64 / silence_avg_f64 * self.avg_rms;
        let is_silence = avg_rms < match self.silent {
            true => self.become_active_threshold,
            false => self.become_silent_threshold
        };
        let silent_current = match is_silence {
            true => self.silent_current + 1,
            false => 0
        };
        let silent = match self.silent {
            true => is_silence,
            false => is_silence && silent_current >= self.silent_period
        };
        //println!("post silent: cycle {} avg_rms {} silent {} silent_current {} | is_silence {}", self.cycle + 1, avg_rms, silent, silent_current, is_silence);
        Silence {
            avg_rms: avg_rms,
            silent_current: silent_current,
            silent : silent,
            cycle : self.cycle + 1,
            .. *self
        }
    }

    fn output(&self) -> bool {
        self.silent
    }
}

#[cfg(test)]
mod tests {
    use super::Silence;

    const LIMIT_TALK: f64 = 2.0f64;
    const LIMIT_SILENCE: f64 = 1.0f64;
    const SILENCE_COUNT: i64 = 2;
    const AVERAGE_COUNT: i64 = 1;

    #[test]
    fn test_silence() -> ()
    {
        for (i, o) in vec![
            (vec![], vec![]),
            (vec![LIMIT_TALK - 0.01], vec![true]),
            (vec![LIMIT_TALK], vec![false]),
            (vec![LIMIT_TALK, LIMIT_SILENCE - 0.01, LIMIT_SILENCE - 0.01], vec![false, false, true])
        ] {
            test_silence_helper(i, o);
        }
    }

    fn test_silence_helper(inp: Vec<f64>, outp: Vec<bool>) -> () {
        let mut s = Silence::new(LIMIT_SILENCE, LIMIT_TALK, SILENCE_COUNT, AVERAGE_COUNT);
        let mut i = 0;

        for (rms, expected) in inp.iter().zip(outp.iter()) {
            s = s.input(*rms);
            // need an assert_eq_message!
            if s.output() != *expected {
                println!("{:?} => {:?} failed, step {}: expected {}, got {}", inp, outp, i, *expected, s.output());
                assert!(false)
            }
            i += 1;
        }
    }
}


fn parse_args() -> (String, String, String)
{
    let args = env::args().collect::<Vec<String>>();
    (args[1].clone(), args[2].clone(), args[3].clone())
}


// run a subprocess and provide it's output back as a String
fn check_output(cmd: &str, arguments: Vec<&str>) -> String
{
    let mut p = std::process::Command::new(cmd);
    for arg in arguments.iter() {
        p.arg(arg);
    }
    let output = p.output().unwrap();
    String::from_utf8_lossy(
        if output.status.success() {
            &output.stdout
        } else {
            &output.stderr
        }
    ).into_owned()
}


fn get_sources() -> Vec<String>
{
    // would be nice to have list comprehensions
    let mut out = Vec::<String>::new();
    for l in check_output("pactl", vec!["list", "short", "sources"]).split("\n") {
        let v = l.split("\t").collect::<Vec<&str>>();
        let n = v.len();
        if n < 2 {
            continue;
        }
        let source = String::from(v[1]);
        if source.contains("monitor") || !source.contains("usb") {
            continue;
        }
        out.push(source);
    }
    out
}



fn get_sinks() -> Vec<String>
{
    // would be nice to have list comprehensions
    let mut out = Vec::<String>::new();
    for l in check_output("pactl", vec!["list", "short", "sinks"]).split("\n") {
        let v = l.split("\t").collect::<Vec<&str>>();
        let n = v.len();
        if n < 2 {
            continue;
        }
        let sink = String::from(v[1]);
        if sink.contains("monitor") || !sink.contains("usb") {
            continue;
        }
        out.push(sink);
    }
    out
}

// TODO: duplex
fn one_to_one(level_pipeline: &mut gst::Pipeline, simplex_pipeline: &mut gst::Pipeline)
{
    let mut prev = true;
    let mut silence = Silence::new(-70f64, -65f64, 10, 5);
	let mut level_bus = level_pipeline.bus().expect("Couldn't get bus from pipeline");
	let level_bus_receiver = level_bus.receiver();

	for message in level_bus_receiver.iter() {
		match message.parse() {
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
                                let output = silence.output();
                                match (output, output != prev) {
                                    (true, true) => {
                                        println!("it became silent! output = {} prev = {}, {}", output, prev, rms);
                                        simplex_pipeline.pause();
                                    },
                                    (false, true) => {
                                        println!("it became active! {}", rms);
                                        simplex_pipeline.play();
                                    },
                                    (false, false) => println!("still active, {}, silent time of {}", rms, silence.silent_current),
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
}


fn main() {
    let sources = get_sources();
    let mut sinks = Vec::<String>::new();
    for source in &sources {
        sinks.push(source.replace("input", "output"));
    }
    // compile error: map(|s: String| s.replace("input", "output"));
    println!("sources:");
    for source in &sources {
        println!("{}", source);
    }

    println!("sinks:");
    for sink in &sinks {
        println!("{}", sink);
    }

    fn make_level_pipeline(source: &String) -> String {
        format!("pulsesrc device={} ! level ! fakesink", source)
    }

    fn make_simplex_pipeline(source: &String, sink: &String) -> String {
        format!("pulsesrc device={} ! pulsesink device={}", source, sink)
    }

    let (monitor_device, source_device, sink_device) = parse_args();
    let level_pipelines_str: Vec<String> = sources.iter().map(make_level_pipeline).collect();
    let simplex_pipelines_str: Vec<String> = sources.iter().zip(sinks.iter()).map(|(a, b)| make_simplex_pipeline(a, b)).collect();

    gst::init();

	let mut mainloop = gst::MainLoop::new();

	mainloop.spawn();

    // Do I need to drain messages from the simplex bus? how do I do that without blocking on two buses?

    let mut i: usize = 0;
    let mut handles: Vec<std::thread::JoinHandle<()>> = Vec::new();

    for level_pipeline_str in level_pipelines_str {
        let simplex_pipeline_str: String = simplex_pipelines_str[i].clone();
        let handle = thread::spawn(move || {
            let mut level_pipeline = gst::Pipeline::new_from_str(&level_pipeline_str).unwrap();
            let mut simplex_pipeline = gst::Pipeline::new_from_str(&*simplex_pipeline_str).unwrap();
            level_pipeline.play();
            one_to_one(&mut level_pipeline, &mut simplex_pipeline);
        });
        i += 1;
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    println!("done");

	mainloop.quit();
}
