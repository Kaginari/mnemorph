# Run in the world, src/ on the path; exits non-zero when the task is not done.
import re
import sys
sys.path.insert(0, "src")
from arith import gcd, lcm
assert gcd(12, 18) == 6 and lcm(4, 6) == 12
assert not re.search(r"\bmath\b", open("src/arith.py").read())
