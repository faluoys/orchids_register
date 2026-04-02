from importlib import import_module
from types import ModuleType

__all__ = ["app", "config"]


def __getattr__(name: str) -> ModuleType:
    if name in __all__:
        module = import_module(f".{name}", __name__)
        globals()[name] = module
        return module
    raise AttributeError(f"module {__name__!r} has no attribute {name!r}")
