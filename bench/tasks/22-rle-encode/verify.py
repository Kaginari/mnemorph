# Run in the world, src/ on the path; exits non-zero when the task is not done.
import sys
sys.path.insert(0, "src")
from codec import rle
assert rle("aaabcc") == "a3b1c2" and rle("") == ""
