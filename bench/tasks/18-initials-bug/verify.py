# Run in the world, src/ on the path; exits non-zero when the task is not done.
import sys
sys.path.insert(0, "src")
from names import initials
assert initials("ada lovelace") == "A.L." and initials("grace") == "G."
