# Run in the world, src/ on the path; exits non-zero when the task is not done.
import sys
sys.path.insert(0, "src")
from text import count_vowels
assert count_vowels("Hello World") == 3 and count_vowels("") == 0
