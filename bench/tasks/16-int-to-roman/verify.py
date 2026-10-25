# Run in the world, src/ on the path; exits non-zero when the task is not done.
import sys
sys.path.insert(0, "src")
from roman import int_to_roman
assert int_to_roman(1994) == "MCMXCIV" and int_to_roman(4) == "IV"
