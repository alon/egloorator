extern crate argparse;
extern crate gst;
extern crate gtk;
extern crate gobject_sys;

use std::process::Command;
use std::env;
use std::thread;
use std::sync::mpsc::{channel, Sender};

use gst::ElementT;
use argparse::{ArgumentParser, StoreTrue, Store, Collect};
//use gtk::prelude::*;

mod silence;
use silence::Silence;

mod gst_helpers;
use gst_helpers::{gst_message_get_double, gst_message_get_name};

mod hub;
use hub::{Hub, SilenceChange};

mod levels;
use levels::{get_levels, get_amplification};


#[derive(Debug)]
enum Message {
    Update(SilenceChange),
    Quit
}


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


fn get_sources(filter_sources: Option<&String>, filter_not_sources:Option<&String>) -> Vec<String>
{
    // would be nice to have list comprehensions
    let mut out = Vec::<String>::new();
    println!("filter sources:      {:?}", filter_sources);
    println!("filter not sources:  {:?}", filter_not_sources);
    for l in check_output("pactl", vec!["list", "short", "sources"]).split("\n") {
        let v = l.split("\t").collect::<Vec<&str>>();
        let n = v.len();
        if n < 2 {
            continue;
        }
        let source = String::from(v[1]);
        if source.contains("monitor") || !source.contains("usb")
            // || !source.contains("Microsoft")
            // || source != "alsa_input.usb-Microsoft_Microsoft_LifeChat_LX-4000-00.analog-stereo"
            // || !source.contains("alsa_input.usb-C-Media_Electronics_Inc._Microsoft_LifeChat_LX-3000")
        {
            continue;
        }
        match filter_sources {
            None => {},
            Some(s) => {
                if !source.contains(s) {
                    continue;
                }
            }
        };

        match filter_not_sources {
            None => {},
            Some(s) => {
                if source.contains(s) {
                    continue;
                }
            }
        }
        out.push(source);
    }
    out
}


const level_interval: f64 = 0.1f64;
static silent_period: i64 = 10 * 30; // 1 seconds
static average_period: i64 = 1; // no averaging - let level element do that
static mut sine_timeout: u64 = (1.0f64 / level_interval) as u64; // 0 for no timeout, i.e. debug mode


fn watch_level(index: usize, level_source: &String, sink: &String, level_pipeline: &mut gst::Pipeline, tx: &Sender<Message>)
{
    let mut prev = true;
    let (s2a, a2s) = get_levels(&level_source);
    println!("{}: s2a {}, a2s {}", level_source, s2a, a2s);
    let mut silence = Silence::new(s2a, a2s, silent_period, average_period);
    let mut level_bus = level_pipeline.bus().expect("Couldn't get bus from pipeline");
    let level_bus_receiver = level_bus.receiver();

    let sine_timeout_max: u64 = unsafe {
        sine_timeout
    };

    let play_sine_on_activity = sink.starts_with("pulsesink");
    let mut sine_timeout_counter = 0u64;
    let mut sine_pipeline = gst::Pipeline::new_from_str("fakesrc ! fakesink").unwrap();
    if play_sine_on_activity {
        let sine_str = format!("ladspasrc-sine-so-sine-fcac amplitude=0.02 ! pulsesink device={}", sink);
        sine_pipeline = gst::Pipeline::new_from_str(sine_str.as_ref()).unwrap();
    }

    for message in level_bus_receiver.iter() {
        match message.parse() {
            gst::Message::StateChangedParsed{ref msg, ref old, ref new, ref pending} => {
                //println!("element `{}` changed from {:?} to {:?}", message.src_name(), old, new);
            }
            gst::Message::ErrorParsed{ref msg, ref error, ref debug} => {
                //println!("error msg from element `{}`: {}, quitting", message.src_name(), error.message());
                break;
            }
            gst::Message::Eos(ref msg) => {
                println!("eos received quiting");
                tx.send(Message::Quit);
                break;
            }
            _ => {
                // level sends messages, look for rms, peak and decay doubles in the structure
                match gst_message_get_name(&message) {
                    Some(the_name) => {
                        if &*the_name == "level" {
                            let rms = gst_message_get_double(&message, "rms");
                            silence = silence.input(rms);
                            println!("{}: {}: rms = {}", the_name, message.src_name(), rms);
                            let output = silence.output();
                            match (output, output != prev) {
                                (true, true) => {
                                    println!("{}: became silent! {}", level_source, rms);
                                    if play_sine_on_activity {
                                        sine_pipeline.pause();
                                    }
                                    tx.send(Message::Update(SilenceChange{who: index, silent: true})).unwrap();
                                },
                                (false, true) => {
                                    println!("{}: became active! {}", level_source, rms);
                                    if play_sine_on_activity {
                                        sine_pipeline.play();
                                        sine_timeout_counter = 5u64; // hardcoded, should be relative to level period, currently 0.1s
                                    }
                                    tx.send(Message::Update(SilenceChange{who: index, silent: false})).unwrap();
                                },
                                _ => {
                                    if play_sine_on_activity {
                                        if sine_timeout_max != 0 && sine_timeout_counter == 0 {
                                            sine_pipeline.pause();
                                        }
                                        if sine_timeout_counter > 0 {
                                            sine_timeout_counter -= 1;
                                        }
                                    }
                                },
                            }
                            prev = output;
                        } else {
                            //println!("ignoring message {}", the_name);
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
    let mut verbose = false;
    let mut filenames: Vec<String> = vec![];
    let mut s2a: f64 = 0.0;
    let mut a2s: f64 = 0.0;
    let mut filter_sources: String = format!("");
    let mut filter_not_sources: String = format!("");
    let mut debug = false;

    {  // this block limits scope of borrows by ap.refer() method
        let mut ap = ArgumentParser::new();
        ap.set_description("Egloorator");
        ap.refer(&mut verbose)
            .add_option(&["-v", "--verbose"], StoreTrue,
            "Be verbose");
        ap.refer(&mut filenames).add_option(&["-f", "--filenames"], Collect, "Filenames");
        ap.refer(&mut s2a).add_option(&["-s", "--s2a"], Store, "Silent to Active");
        ap.refer(&mut a2s).add_option(&["-a", "--a2s"], Store, "Active to Silent");
        ap.refer(&mut filter_sources).add_option(&["-i", "--filter-sources"], Store, "Filter sources");
        ap.refer(&mut filter_not_sources).add_option(&["-x", "--filter-not-sources"], Store, "Filter sources");
        ap.refer(&mut debug).add_option(&["-d", "--debug"], StoreTrue, "debug (turn on sine sound)");
        ap.parse_args_or_exit();
    }

    if debug {
        unsafe {
            sine_timeout = 0u64;
        }
    }

    println!("using level.interval of {}", level_interval);
    unsafe {
        println!("using sine timeout of {}", sine_timeout);
    }

    let source_devices = get_sources(if filter_sources.len() == 0 { None } else { Some(&filter_sources) }, if filter_not_sources.len() == 0 { None } else { Some(&filter_not_sources) });
    let sources: Vec<String> = match filenames.len() {
        0 => source_devices.iter().map(|s| format!("pulsesrc device={}", s)).collect(),
        _ => filenames.iter().map(|f| format!("filesrc location={} ! wavparse", f)).collect(),
    };
    let mut sinks: Vec<String> =
        if filenames.len() == 0 {
            sources.iter().map(|s: &String| format!("pulsesink device={}",
                                     s.replace("input", "output").replace("mono", "stereo"))).collect()
        } else {
            (0..(filenames.len())).map(|i| format!("filesink location=output_{}.wav", i)).collect()
        };
    // compile error: map(|s: String| s.replace("input", "output"));
    println!("{} sources:", sources.len());
    for source in &sources {
        println!("{}", source);
    }

    println!("{} sinks:", sinks.len());
    for sink in &sinks {
        println!("{}", sink);
    }

    fn source_pipelines() {

    }

    fn make_level_pipeline(source: &String) -> String {
        format!("{} ! level interval={} ! fakesink", source, level_interval)
    }

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
        let mut hub = Hub::new(&sources, &sinks);

        for msg in rx {
            println!("sending {:?} to hub", msg);
            match msg {
                Message::Update(silence_change) => hub.input(&silence_change),
                Message::Quit => break,
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
