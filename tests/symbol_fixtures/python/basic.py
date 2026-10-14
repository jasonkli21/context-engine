@decorator
class Router:
    @traced
    async def route(self):
        def inner():
            pass
    class Inner:
        def method(self):
            pass

def outer():
    @decorator
    def inner():
        pass
    class Local:
        def run(self):
            pass

async def top_level():
    pass

def café():
    é = 0; naïve = 1
    def naïve():
        pass
