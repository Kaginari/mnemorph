# Run in the world, src/ on the path; exits non-zero when the task is not done.
import sys
sys.path.insert(0, "src")
from lists import dedupe
assert dedupe([3, 1, 3, 2, 1]) == [3, 1, 2]
