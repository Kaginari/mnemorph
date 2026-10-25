# Run in the world, src/ on the path; exits non-zero when the task is not done.
import sys
sys.path.insert(0, "src")
from units import c_to_f
assert c_to_f(100) == 212 and c_to_f(-40) == -40
