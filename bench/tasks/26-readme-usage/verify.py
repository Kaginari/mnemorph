# Run in the world, src/ on the path; exits non-zero when the task is not done.
import sys
sys.path.insert(0, "src")
text = open("README.md").read()
assert "## Usage" in text and "greet" in text.split("## Usage", 1)[1]
