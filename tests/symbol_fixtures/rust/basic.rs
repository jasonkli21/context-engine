const DEFAULT: usize = 1;
static ENABLED: bool = true;
type Name = String;
#[derive(Debug)]
struct Engine<T> { value: T }
enum Mode { Fast }
union Bits { value: u32 }
trait Analyzer {
    type Output;
    fn analyze(&self);
    fn defaulted(&self) {}
}
impl<T> Engine<T> {
    const VALUE: usize = 1;
    fn run(&self) { fn inner() {} }
}
impl<T> Analyzer for Engine<T> {
    type Output = T;
    fn analyze(&self) {}
}
fn top_level() {}
mod parser {
    mod nested { pub fn parse() {} }
}
fn r#type() {}
fn café() { let é = 0; fn naïve() {} }
