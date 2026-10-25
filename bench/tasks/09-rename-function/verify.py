# Run in the world, src/ on the path; exits non-zero when the task is not done.
import sys
sys.path.insert(0, "src")
import cart, report
assert not hasattr(cart, "calc_total") and cart.total([1, 2]) == 3
assert report.summary([2, 3]) == "total: 5"
