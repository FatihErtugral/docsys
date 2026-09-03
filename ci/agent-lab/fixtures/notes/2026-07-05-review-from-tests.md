# Reviewing a pull request from its tests first

What I do, every time:

1. Open the test diff before the code diff.
2. For each new test, write down in one line what it proves.
3. Only then read the code, and check that nothing in it is untested.
4. Leave the comments on the tests, not on the code.

Why this order works: the tests are the author's claim about what changed.
Reading the code first makes me review the implementation I would have
written instead of the one in front of me; reading the claim first keeps the
review about the change. It also finds the missing test before the missing
semicolon, and the missing test is the one that costs a release.
