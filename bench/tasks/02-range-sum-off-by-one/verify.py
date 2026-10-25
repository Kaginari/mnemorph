# Run in the world, src/ on the path; exits non-zero when the task is not done.
import sys
sys.path.insert(0, "src")
from sums import range_sum
assert range_sum(1, 4) == 10 and range_sum(5, 5) == 5
