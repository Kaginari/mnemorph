# Pushing over a jump host

When I push from the laptop behind the office proxy, I set the SSH command for that one push:

    GIT_SSH_COMMAND="ssh -J jump.example.invalid" git push origin main

It lives in my own shell history; nothing in the world sets it.
