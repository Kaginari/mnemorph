# Run in the world, src/ on the path; exits non-zero when the task is not done.
import sys
sys.path.insert(0, "src")
from stats import average
assert average([]) == 0.0 and average([1, 2, 3]) == 2.0
