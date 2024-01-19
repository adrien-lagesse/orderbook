pub struct IDGenerator {
    i: i32,
}

impl IDGenerator {
    pub fn new() -> IDGenerator {
        IDGenerator { i: -1 }
    }

    pub fn next(&mut self) -> i32 {
        let res = self.i;
        self.i += 1;
        res
    }
}
