# Run in the world, src/ on the path; exits non-zero when the task is not done.
import sys
sys.path.insert(0, "src")
from lists import flatten
assert flatten([1, [2, [3, [4]], 5], []]) == [1, 2, 3, 4, 5]
