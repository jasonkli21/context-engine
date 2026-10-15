use foo::bar;
use foo::bar as baz;
use foo::{bar, nested::{Thing, Other as Alias}};
use foo::*;
use crate::x::Y;
use self::x;
use super::x;
struct Foo {}
impl Foo { fn method() { helper(); } }
mod inner { fn nested() { super::helper(); } }
fn helper() { helper(); }
fn caller() { helper(); helper(); inner::nested(); obj.method(); (factory())(); items[0](); }
