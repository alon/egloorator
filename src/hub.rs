extern crate gst;
use gst::Pipeline;
use gst::ElementT;


#[derive(Debug)]
pub struct SilenceChange {
    pub who: usize,
    pub silent: bool,
}


pub struct Hub {
    pipes: Vec<Vec<Pipeline>>,
}


impl Hub {
    pub fn new(pipes: Vec<Vec<Pipeline>>) -> Hub
    {
        Hub {pipes: pipes}
    }

    // This also toggles all of the pipelines. It would be nicer if we could do this
    // via gstreamer, as a control flow? my ascii art fails me. Something like:
    // hub -> [play_bit(pipeline) for pipeline in pipelines]
    pub fn input(&mut self, msg: &SilenceChange)
    {
        println!("got {:?}", msg);
        for mut p in &mut self.pipes[msg.who] {
            if msg.silent {
                p.pause();
            } else {
                p.play();
            }
        }
    }
}
