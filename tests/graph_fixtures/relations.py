import foo
import foo.bar as fb
from foo import A, B as Alias
from . import sibling
from ..pkg import Name
from foo import *
class Outer:
    def helper(self):
        pass
    def method(self):
        import local
        Outer.helper()
        helper()
        self.helper()
        factory()()
        items[0]()
def helper():
    helper()
def caller():
    helper()
    helper()
    Outer.helper()
