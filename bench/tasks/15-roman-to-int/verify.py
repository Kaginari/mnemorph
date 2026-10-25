# Run in the world, src/ on the path; exits non-zero when the task is not done.
import sys
sys.path.insert(0, "src")
from roman import roman_to_int
assert roman_to_int("MCMXCIV") == 1994 and roman_to_int("III") == 3
