# Run in the world, src/ on the path; exits non-zero when the task is not done.
import sys
sys.path.insert(0, "src")
from stack import Stack
s = Stack(); s.push(1); s.push(2)
assert s.peek() == 2 and len(s) == 2 and s.pop() == 2 and s.pop() == 1
try:
    s.pop(); raise SystemExit(1)
except IndexError:
    pass
