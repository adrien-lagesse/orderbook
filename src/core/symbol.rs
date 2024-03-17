#[derive(Clone, Hash, Eq, PartialEq, Debug)]
pub struct Spot {
    pub base: &'static str,
    pub quote: &'static str,
}

impl ToString for Spot {
    fn to_string(&self) -> String {
        format!("{0}{1}", self.quote, self.base)
    }
}
