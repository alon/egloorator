extern crate argparse;
extern crate gst;
extern crate gtk;
extern crate gobject_sys;

use std::process::Command;
use std::env;
use std::thread;
use std::sync::mpsc::{channel, Sender};

use gst::ElementT;
//use argparse::ArgumentParser;
//use gtk::prelude::*;

mod silence;
use silence::Silence;
mod gst_helpers;
use gst_helpers::{gst_message_get_double, gst_message_get_name};


// run a subprocess and provide it's output back as a String
fn check_output(cmd: &str, arguments: Vec<&str>) -> String
{
    let mut p = Command::new(cmd);
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
        if source.contains("monitor") || !source.contains("usb")
            || !source.contains("Microsoft")
            // || source != "alsa_input.usb-Microsoft_Microsoft_LifeChat_LX-4000-00.analog-stereo"
            // || !source.contains("alsa_input.usb-C-Media_Electronics_Inc._Microsoft_LifeChat_LX-3000")
        {
            continue;
        }
        out.push(source);
    }
    out
}


// return S->A level, A->S level (larger first)
fn get_levels(source: &String) -> (f64, f64)
{
    if source.contains("Logitech") {
        return (-40f64, -41f64);
    }
    if source == "alsa_input.usb-C-Media_Electronics_Inc._Microsoft_LifeChat_LX-3000-00.analog-mono.2" {
        return (-40f64, -45f64)
    }
    if source == "alsa_input.usb-C-Media_Electronics_Inc._Microsoft_LifeChat_LX-3000-00.analog-mono" {
        return (-40f64, -45f64)
    }
    if source == "alsa_input.usb-Microsoft_Microsoft_LifeChat_LX-4000-00.analog-stereo" {
        return (-40f64, -45f64)
    }
    (-70f64, -65f64)
}


fn watch_level(index: usize, level_source: &String, sink: &String, level_pipeline: &mut gst::Pipeline, tx: &Sender<SilenceChange>)
{
    let mut prev = true;
    let (s2a, a2s) = get_levels(&level_source);
    let mut silence = Silence::new(s2a, a2s, 10, 5);
	let mut level_bus = level_pipeline.bus().expect("Couldn't get bus from pipeline");
	let level_bus_receiver = level_bus.receiver();

    let sine_str = format!("ladspasrc-sine-so-sine-fcac amplitude=0.02 ! pulsesink device={}", sink);
    let mut sine_pipeline = gst::Pipeline::new_from_str(sine_str.as_ref()).unwrap();

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
                                //println!("{}: rms: {}", silence.cycle, rms);
                                silence = silence.input(rms);
                                let output = silence.output();
                                match (output, output != prev) {
                                    (true, true) => {
                                        println!("{}: became silent! {}", level_source, rms);
                                        sine_pipeline.pause();
                                        tx.send(SilenceChange{who: index, silent: true}).unwrap();
                                    },
                                    (false, true) => {
                                        println!("{}: became active! {}", level_source, rms);
                                        sine_pipeline.play();
                                        tx.send(SilenceChange{who: index, silent: false}).unwrap();
                                    },
                                    (false, false) => {}, // println!("still active, {}, silent time of {}", rms, silence.silent_current),
                                    (true, false) => {}, // println!("still silent"),
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


#[derive(Debug)]
struct SilenceChange {
    who: usize,
    silent: bool,
}


fn main() {
    let sources = get_sources();
    let mut sinks = Vec::<String>::new();
    for source in &sources {
        sinks.push(source.replace("input", "output").replace("mono", "stereo"));
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

    let source_n = sources.len();
    let sink_n = sinks.len();

    gst::init();

	let mut mainloop = gst::MainLoop::new();

	mainloop.spawn();

    let mut handles: Vec<std::thread::JoinHandle<()>> = Vec::new();
    let (tx, rx) = channel();

    for (i, (orig_source, orig_sink)) in sources.iter().zip(sinks.clone()).enumerate() {
        let source = orig_source.clone();
        let sink = orig_sink.clone();
        let tx = tx.clone();
        let handle = thread::spawn(move || {
            let level_pipeline_str = make_level_pipeline(&source);
            let mut level_pipeline = gst::Pipeline::new_from_str(&level_pipeline_str).unwrap();
            level_pipeline.play();
            watch_level(i, &source, &sink, &mut level_pipeline, &tx);
        });
        handles.push(handle);
    }

    let coordinator = thread::spawn(move || {
        // I have all those pipelines
        // I keep track of non silent types and connect them directly
        let mut source_to_sink: Vec<Vec<gst::Pipeline>> = Vec::new();

        for source_i in 0..source_n {
            source_to_sink.push(Vec::new());
            for sink_i in 0..sink_n {
                let s = make_simplex_pipeline(&sources[source_i], &sinks[sink_i]);
                source_to_sink[source_i].push(gst::Pipeline::new_from_str(&*s).unwrap());
            }
        }

        for msg in rx {
            println!("got {:?}", msg);
            for mut p in &mut source_to_sink[msg.who] {
                if msg.silent {
                    p.pause();
                } else {
                    p.play();
                }
            }
        }
    });

    coordinator.join().unwrap();

    for handle in handles {
        handle.join().unwrap();
    }

    println!("done");

	mainloop.quit();
}
