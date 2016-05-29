use std::collections::HashSet;

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


type Voice = usize;


// This is the logic - mut free for easy testing
struct Egloorator {
    silent: HashSet<Voice>,
    single: Option<Voice>,
    pairs: HashSet<(Voice, Voice)>
}


impl Egloorator {

    fn new(start: Vec<bool>) -> Egloorator {
        let silent = (0..start.len()).collect();
        let mut er = Egloorator {
            silent: silent,
            single: None,
            pairs: HashSet::<(Voice, Voice)>::new()
        };
        for (i, silent) in start.iter().enumerate() {
            if !silent {
                er = er.input(&SilenceChange {
                    who: i,
                    silent: false
                });
            }
        }
        er
    }

    fn input(self, change: &SilenceChange) -> Egloorator {
        // TODO
        self
    }
}


#[cfg(test)]
mod tests {
    use super::Egloorator;

    #[test]
    fn test_sanity() {
        // TODO
    }
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
