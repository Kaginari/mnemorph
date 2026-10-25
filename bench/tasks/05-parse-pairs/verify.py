# Run in the world, src/ on the path; exits non-zero when the task is not done.
import sys
sys.path.insert(0, "src")
from config import parse_pairs
assert parse_pairs(" a = 1 ;b=2;;") == {"a": "1", "b": "2"} and parse_pairs("") == {}
