# Run in the world, src/ on the path; exits non-zero when the task is not done.
import sys
sys.path.insert(0, "src")
import json
c = json.load(open("data/config.json"))
assert c["name"] == "bench" and c["retries"] == 3
