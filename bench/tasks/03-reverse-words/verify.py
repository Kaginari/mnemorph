# Run in the world, src/ on the path; exits non-zero when the task is not done.
import sys
sys.path.insert(0, "src")
from text import reverse_words
assert reverse_words("a  quick brown fox") == "fox brown quick a"
