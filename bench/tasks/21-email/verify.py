# Run in the world, src/ on the path; exits non-zero when the task is not done.
import sys
sys.path.insert(0, "src")
from validate import is_email
assert is_email("a.b@example.org") and not is_email("ab.example.org") and not is_email("a@b")
