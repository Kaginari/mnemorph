# Run in the world, src/ on the path; exits non-zero when the task is not done.
import sys
sys.path.insert(0, "src")
from codec import caesar
assert caesar("Hello, World!", 3) == "Khoor, Zruog!" and caesar("abc", -1) == "zab"
