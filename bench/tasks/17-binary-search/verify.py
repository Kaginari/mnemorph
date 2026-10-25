# Run in the world, src/ on the path; exits non-zero when the task is not done.
import sys
sys.path.insert(0, "src")
from search import index_of
xs = list(range(0, 100, 3))
assert index_of(xs, 27) == 9 and index_of(xs, 28) == -1 and index_of([], 1) == -1
