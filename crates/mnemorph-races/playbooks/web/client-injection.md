### client-injection
Invariant: model, tool and repository text renders in the board as text, never as markup or script.
Look: rendering of transcripts, markdown, diffs and file names; `innerHTML`, raw-HTML options of markdown renderers, link `href`s, the CSP.
Check: put `<img onerror>`, a `javascript:` link, SVG and raw HTML in each rendered field.
Witness: a test rendering a crafted message and asserting script or markup reaches the DOM.
Not a finding: escaped output; markup the board authors itself.
