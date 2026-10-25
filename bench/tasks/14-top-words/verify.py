# Run in the world, src/ on the path; exits non-zero when the task is not done.
import sys
sys.path.insert(0, "src")
from text import top_words
assert top_words("b a B c a b", 2) == [("b", 3), ("a", 2)]
