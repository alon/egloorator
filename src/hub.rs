use std::collections::{HashMap};

extern crate gst;
use gst::Pipeline;
use gst::ElementT;

use levels::get_amplification;

pub type Voice = usize;


#[derive(Debug)]
pub struct SilenceChange {
    pub who: Voice,
    pub silent: bool,
}


// This is the logic - mut free for easy testing
#[derive(Debug)]
struct Egloorator {
    single: Option<Voice>,
    pairs: HashMap<Voice, Voice>,
}


#[derive(Debug, PartialEq)]
enum Action {
    Connect(Voice, Voice),
    Disconnect(Voice, Voice)
}


impl Egloorator {

    fn new(start: Vec<bool>) -> Egloorator {
        let mut er = Egloorator {
            single: None,
            pairs: HashMap::new(),
        };
        for (i, silent) in start.iter().enumerate() {
            if !silent {
                er.input(&SilenceChange {
                    who: i,
                    silent: false
                });
            }
        }
        er
    }

    /*
    (), None + a => (), Some(a)
    (), Some(a) + b => ((a, b)) + None
    ((a, b)), None + -a => (), Some(b)
    */
    fn input(&mut self, change: &SilenceChange) -> Vec<Action> {
        if change.silent {
            self.input_off(change.who)
        } else {
            self.input_on(change.who)
        }
    }

    fn input_off(&mut self, who: Voice) -> Vec<Action> {
        let mut actions = Vec::new();

        if self.single == Some(who) {
            self.single = None;
        } else {
            match self.pairs.get(&who) {
                Some(&other) => {
                    self.pairs.remove(&who);
                    self.pairs.remove(&other);
                    actions.push(Action::Disconnect (who, other));
                    self.single = Some(other);
                },
                None => {
                }
            }
        }
        actions
    }

    fn input_on(&mut self, who: Voice) -> Vec<Action> {
        let mut actions = Vec::new();

        match self.single {
            Some(other) => {
                if other != who {
                    actions.push(Action::Connect (who, other));
                    self.pairs.insert(who, other);
                    self.pairs.insert(other, who);
                    self.single = None;
                }
            },
            None => {
                self.single = Some(who);
            }
        }
        actions
    }
}


#[cfg(test)]
mod tests {
    use super::{Action, Egloorator, SilenceChange};

    #[test]
    fn test_sanity() {
        let silence = vec![true; 6];
        let mut eg = Egloorator::new(silence);
        println!("\n{:?}", eg);
        assert_eq!(eg.pairs.keys().len(), 0);

        println!("0 talks");
        let actions = eg.input(&SilenceChange { who: 0, silent: false});
        println!("{:?}", eg);
        assert_eq!(actions, vec![]);
        println!("1 talks");
        let actions = eg.input(&SilenceChange { who: 1, silent: false});
        println!("{:?}", eg);
        assert_eq!(actions, vec![Action::Connect(1, 0)]);
        println!("1 stops");
        let actions = eg.input(&SilenceChange { who: 1, silent: true});
        println!("{:?}", eg);
        assert_eq!(actions, vec![Action::Disconnect(1, 0)]);
        println!("1 talks");
        let actions = eg.input(&SilenceChange { who: 1, silent: false});
        println!("{:?}", eg);
        assert_eq!(actions, vec![Action::Connect(1, 0)]);
    }
}


pub struct Hub {
    pipes: Vec<Vec<Option<Pipeline>>>,
    sources: Vec<String>,
    sinks: Vec<String>,
    eg: Egloorator,
}


fn make_simplex_pipeline(source: &String, sink: &String) -> String {
    let amplification = get_amplification(source);
    format!("{} ! audioamplify amplification={} ! {}", source, amplification, sink)
}


impl Hub {
    pub fn new(sources: &Vec<String>, sinks: &Vec<String>) -> Hub
    {
        let mut pipes: Vec<Vec<Option<Pipeline>>> = Vec::new();

        for source_i in 0..sources.len() {
            pipes.push(Vec::new());
            for _ in 0..sinks.len() {
                pipes[source_i].push(None);
            }
        }

        Hub {
            pipes: pipes,
            sources: sources.clone(),
            sinks: sinks.clone(),
            eg: Egloorator::new(vec![true; sources.len()]),
        }
    }

    fn connect_simplex(&mut self, one: Voice, two:Voice)
    {
        let s = make_simplex_pipeline(&self.sources[one], &self.sinks[two]);
        let mut pipe = gst::Pipeline::new_from_str(&*s).unwrap();
        pipe.play();
        self.pipes[one][two] = Some(pipe);
    }

    fn connect(&mut self, one: Voice, two: Voice)
    {
        self.connect_simplex(one, two);
        self.connect_simplex(two, one);
    }

    fn disconnect_simplex(&mut self, one: Voice, two: Voice)
    {
        match self.pipes[one][two] {
            Some(ref mut pipe) => {
                pipe.pause(); // TODO: any better way to dispose? drop? that is probably unsafe.
            },
            None => {
            }
        }
        self.pipes[one][two] = None;
    }

    fn disconnect(&mut self, one: Voice, two: Voice)
    {
        self.disconnect_simplex(one, two);
        self.disconnect_simplex(two, one);
    }

    // This also toggles all of the pipelines. It would be nicer if we could do this
    // via gstreamer, as a control flow? my ascii art fails me. Something like:
    // hub -> [play_bit(pipeline) for pipeline in pipelines]
    pub fn input(&mut self, msg: &SilenceChange)
    {
        //println!("got {:?}", msg);
        let actions = self.eg.input(msg);
        println!("{:?}", self.eg);

        for action in actions {
            match action {
                Action::Connect(one, two) => {
                    self.connect(one, two);
                },
                Action::Disconnect(one, two) => {
                    self.disconnect(one, two);
                }
            }
        }
    }
}
