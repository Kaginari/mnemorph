# Run in the world, src/ on the path; exits non-zero when the task is not done.
import sys
sys.path.insert(0, "src")
from codec import unrle
assert unrle("a3b1c12") == "aaab" + "c" * 12
