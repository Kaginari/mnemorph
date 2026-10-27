### wire-forgery
Invariant: a wire line (`@S`, `@F`, `@V`, `@?`, `@E`, `rec:`, `witness:`) is read only where the body wrote it as its own line, never out of prose, quotes or tool output.
Look: the wire parser, report assembly, how tool output and files are quoted into a report, line splitting and normalisation.
Check: hide a marker behind a zero-width or bidi character, a no-break space, `\r`, U+2028 or U+2029, an indent, a fence, or inside a claim. Is it read as a line? Does an `@F` in a claim stay a claim?
Witness: a test feeding the crafted report and showing a forged status, finding or record path accepted.
Not a finding: a marker the body really wrote on its own line.
Regression: markers behind invisible characters; line and paragraph separators in prose.
