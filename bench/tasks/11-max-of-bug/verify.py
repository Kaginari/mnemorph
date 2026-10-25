# Run in the world, src/ on the path; exits non-zero when the task is not done.
import sys
sys.path.insert(0, "src")
from stats import max_of
assert max_of([-5, -2, -9]) == -2 and max_of([3, 7, 1]) == 7
