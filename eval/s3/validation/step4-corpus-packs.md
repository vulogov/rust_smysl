# S3 validation, step 4: is the prerequisite in the corpus, and does it pack?

Checked 2026-09-17 with smysl 1.3.0 `pack --budget 4000 --focus <prerequisite> --focus <decision>`
against the research extraction (research-pro-v2) of the commit where each prerequisite originates.

| # | Source commit | Prerequisite unit | Decision unit | Packed | Tokens used |
|---|---|---|---|---|---|
| 1 | smysl 532e4d2 | `p/g532e4d2-2-1` "The guard only triggers when the parser reads back a different result from write_surface." | `d/g532e4d2-2` (decline) "Leave the round-trip guard untested." | 34 of 34 units | 989 |
| 2 | smysl 90ec2f7 | `p/g90ec2f7-8-1` "The fmt and check commands read from stdin when no path is provided." | `d/g90ec2f7-8` | 53 of 53 | 1467 |
| 3 | smysl 90ec2f7 | `p/g90ec2f7-6-1` "The ingest and usage commands write to files in the working directory." | `d/g90ec2f7-6` | 53 of 53 | 1467 |
| 4 | smysl 532e4d2 | `p/g532e4d2-3-1` "The mutation testing tool reuses build directories across test runs." | `d/g532e4d2-3` "Build the required document in each test instead of using repository fixtures." | 34 of 34 | 989 |
| 5 | smysl 90ec2f7 | `p/g90ec2f7-3-2` "Clap rejects commands missing required arguments before routing occurs." | `d/g90ec2f7-3` | 53 of 53 | 1467 |
| 6 | smysl 4968383 | `p/g4968383-12-1` "Rule M compares statuses as integers." | `d/g4968383-12` "Make status integers normative in the spec." | 89 of 89 | 2571 |
| 7 | ucal 2074788 | `p/g2074788-4-1` "No clock is read in the pure function." | `d/g2074788-4` | 63 of 63 | 1615 |
| 8 | ucal 2074788 | `p/g2074788-3-1` "A forward jump is a correction arriving." | `d/g2074788-3` | 63 of 63 | 1615 |
| 9 | ucal 5880e47 | `p/g5880e47-1-2` "Other parts of the codebase treat versions as opaque strings." | `d/g5880e47-2` (decline) "Do not sort release notes filenames numerically." | 27 of 27 | 780 |
| 10 | ucal 5880e47 | `p/g5880e47-3-1` "A cycle's release notes are created when the cycle opens." | `d/g5880e47-3` | 27 of 27 | 780 |
| 11 | ucal cc3aafe | `p/gcc3aafe-1-2` "A committed fixture cannot race with anything." | `d/gcc3aafe-1` | 26 of 26 | 661 |
| 12 | ucal 2074788 | `p/g2074788-8-1` "A tick count is unsigned by Rule B." | `d/g2074788-8` | 63 of 63 | 1615 |

**Result:** every prerequisite is in the corpus, and each originating commit's whole store fits the 4k
budget with room to spare.

**Caveat for the run:** a whole single-commit store fitting means the budget did not bind, so this does not
yet test packing. The S3 corpus should hold every extracted commit that touches the task's files. When
that store is built, repeat this check: the focused prerequisite and its decision must survive a binding
budget, together with their rebuttals (pack rule R).

**Caveat on quality:** the corpus is the research-pro-v2 extraction, not owner labels. Using it in S3 tests
the product as it would ship, extraction errors included; S0 scores those errors separately.
