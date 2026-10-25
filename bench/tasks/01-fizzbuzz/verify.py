# Run in the world, src/ on the path; exits non-zero when the task is not done.
import sys
sys.path.insert(0, "src")
from kata import fizzbuzz
assert [fizzbuzz(i) for i in (1,3,5,15,7)] == ["1","Fizz","Buzz","FizzBuzz","7"]
