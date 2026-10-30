import os
from typing import List

MAX = 10


def top(x):
    return x


class Stack:
    """A stack."""

    def __init__(self):
        self.items = []

    @property
    def size(self):
        return len(self.items)

    class Inner:
        def deep(self):
            pass


@decorator
def wrapped():
    pass
