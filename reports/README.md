# Reports Directory

This directory intentionally does not store generated evidence artifacts in Git.

Historical report files are evidence artifacts referenced by hash in the
whitepaper Appendix A. They have been archived outside the repository under
`~/repos/postfiat-archive/postfiatl1v2/` so the source tree stays small while
the evidence remains available for audit and hash reconciliation.

The repository `.gitignore` keeps generated report outputs from being re-added:
`reports/*` is ignored, with explicit exceptions for `reports/.gitkeep` and this
README.
