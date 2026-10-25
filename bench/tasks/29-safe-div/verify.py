# Run in the world, src/ on the path; exits non-zero when the task is not done.
import sys
sys.path.insert(0, "src")
from numbers2 import safe_div
assert safe_div(6, 3) == 2
try:
    safe_div(1, 0); raise SystemExit(1)
except ValueError as e:
    assert str(e) == "division by zero"
