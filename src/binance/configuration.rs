
pub enum Schedule {
    Balanced(usize),
    MaxPerTask(usize),
}

pub struct Configuration {
    pub num_threads : usize,
    pub scheduling_strategy: Schedule,
}

impl Configuration {
    pub fn new() -> Configuration {
        Configuration {
            num_threads: 1,
            scheduling_strategy: Schedule::Balanced(1)
        }
    }
}