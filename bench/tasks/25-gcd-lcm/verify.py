# Run in the world, src/ on the path; exits non-zero when the task is not done.
import sys
sys.path.insert(0, "src")
from importlib import util
spec = util.spec_from_file_location("nums", "src/numbers.py"); m = util.module_from_spec(spec); spec.loader.exec_module(m)
assert m.gcd(12, 18) == 6 and m.lcm(4, 6) == 12
assert "import math" not in open("src/numbers.py").read()
