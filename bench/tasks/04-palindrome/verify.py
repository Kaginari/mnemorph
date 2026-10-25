# Run in the world, src/ on the path; exits non-zero when the task is not done.
import sys
sys.path.insert(0, "src")
from text import is_palindrome
assert is_palindrome("A man, a plan, a canal: Panama") and not is_palindrome("abc")
