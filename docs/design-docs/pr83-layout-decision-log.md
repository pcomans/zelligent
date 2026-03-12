# PR 83 Layout Decision Log

This file records the question-and-answer thread used to clarify the intended
design and review criteria for the sidebar layout follow-up.

## 1. Shell default

Q: Should `zelligent` with no args still honor the user's shell default exactly
like `zelligent spawn` does, or intentionally force `bash`?

A: Honor the shell default.

## 2. Missing sidebar plugin at startup

Q: When the persistent sidebar plugin cannot be resolved at startup, should
`zelligent` fail with a clear error, or fall back to a plain Zellij session
with no zelligent UI?

A: Fail with a clear error.

## 3. Custom layout scope

Q: Under the new sidebar model, should a repo-local `.zelligent/layout.kdl`
still customize the session created by plain `zelligent`, or is custom layout
support only meant for `spawn` tabs?

A: It should always be the same.

## 4. Can one file drive everything?

Q: Can we add the plugin pane to `.zelligent/layout.kdl` and have things always
100% the same?

A: Yes in spirit, but not by naively inlining a full sidebar pane in every path.
The resulting decision was that the layout contract needs to be explicit and
shared across initial tabs, spawn tabs, and manual tabs.

## 5. Meaning of `.zelligent/layout.kdl`

Q: Do you want `.zelligent/layout.kdl` to mean "the entire tab layout," or
"just the inner content area inside zelligent's fixed sidebar and status-bar
frame"?

A: The engineering direction is that it should be the inner content area, but
only if the contract is made explicit and documented.

## 6. Breaking contract change

Q: Do you want to officially change `.zelligent/layout.kdl` semantics to mean
"inner content only," and accept that existing full-layout custom files will
need to be migrated?

A: Yes and yes. No migration concerns because there are no users yet, and the
docs must document this decision.

## 7. Initial tab body

Q: When plain `zelligent` creates the initial repo tab, should its inner body be
the same two-pane layout as a spawned tab, just rooted at the main repo
(`$SHELL` and `lazygit`), or a different initial-tab body?

A: The same layout, with the tab bar on the left.

Note: "tab bar on the left" here refers to the persistent zelligent sidebar
replacing the old floating or tab-bar model.

## 8. Manual tabs created directly in Zellij

Q: Should manually created tabs in the session also get that same default inner
body, or only inherit the left sidebar frame and leave the body empty or
default?

A: The intent was "everything should be the same." Later clarified more
precisely:

- Manual tabs created directly in Zellij should inherit the zelligent sidebar
  frame.
- If Zellij does not allow full control over manual-tab bodies, it is
  acceptable for the inner body to remain Zellij's default.
- If that limitation exists, it must be documented once verified.

## 9. Manual-tab cwd

Q: For manual tabs, should the body open in the main repo root, or in whatever
cwd Zellij would otherwise use for that tab?

Initial answer: Whatever Zellij would normally use.

Later simplification decision:

- `{{cwd}}` for manual tabs should resolve to the repo root.
- This simpler deterministic rule replaces "whatever Zellij would normally
  assign."

## 10. Sidebar chrome mandatory

Q: Should the sidebar and plugin chrome be mandatory even when
`.zelligent/layout.kdl` exists, meaning custom layouts can only change the inner
body and can never remove the sidebar and status bar?

A: Yes.

## 11. Old `Ctrl-y` keybinding handling

Q: When `zelligent doctor` runs on a machine that already has the old `Ctrl-y`
floating-plugin keybinding from a previous version, should it remove that stale
keybinding automatically, or just stop adding it and leave existing config
untouched?

A: Just stop adding it and leave existing config untouched.

## 12. No-args startup requirement

Q: Should the no-args path refuse to start unless the sidebar plugin is
resolvable even if `zellij` itself is installed, with an error that explicitly
tells the user to run `zelligent doctor` or reinstall the plugin?

A: Yes. If someone doesn't want the sidebar they can just start Zellij
directly.

## 13. Sidebar rewrite plan doc

Q: Should `docs/design-docs/sidebar-rewrite-plan.md` remain in the repo as an
implementation and history document, or be deleted once the real source-of-truth
docs are updated?

A: It should stay. In the spirit of harness engineering it needs to remain, but
it must accurately document what was built.

## 14. Custom layout freedom

Q: Should `.zelligent/layout.kdl` be allowed to replace the entire inner body
arbitrarily, or should zelligent still enforce the standard `$SHELL` and
`lazygit` two-pane body and only allow smaller tweaks inside that?

A: People can do whatever they want in there.

## 15. Custom layout applies everywhere

Q: When a repo provides `.zelligent/layout.kdl`, should that custom inner body
apply everywhere equally: plain `zelligent` initial tab, `zelligent spawn` tabs,
and manual tabs created directly in Zellij inside that session, all wrapped in
the fixed sidebar chrome?

A: Yes.

## 16. Backward compatibility

Q: Should we preserve support for old full-layout custom files for a while, or
switch cleanly to the new contract with no backward-compat shim?

A: No backward compatibility needed.

## 17. File format: full layout vs fragment

Q: Should `.zelligent/layout.kdl` stay a normal KDL document with an outer
`layout { ... }`, or become a raw inner-body fragment?

Interim exploration:

- Fragment was considered because it is semantically honest.
- Full `layout { ... }` was considered because it is more boring and legible.

Final resolution came via placeholders instead:

- Keep `.zelligent/layout.kdl` as a full layout file.
- Do not use "strip the wrapper and inject guts" as the contract.

## 18. Sidebar placeholder

Q: Can we add a placeholder for the zelligent sidebar?

A: Yes. This became the chosen design direction.

## 19. Status bar terminology

Q: What is the "zelligent status bar"?

A: It is just the normal Zellij status bar. The earlier label was wrong.

## 20. Required placeholders

Q: Should both sidebar and status-bar placeholders be mandatory?

A: No. Only the sidebar placeholder is mandatory. If someone wants to get rid of
the status bar, that is allowed.

## 21. Placeholder set

Q: Should the placeholder contract be minimal?

A: Yes. The intended supported placeholders are:

- `{{zelligent_sidebar}}` required
- `{{cwd}}` optional
- `{{agent_cmd}}` optional

Unknown placeholders should be left alone and whatever Zellij does with them is
fine.

## 22. Built-in layout vs installed default

Q: Should the default built-in layout also be expressed through the same
placeholder mechanism internally so custom and default paths share one rendering
pipeline?

A: Yes, and there really should not be a built-in layout in the script. The
layout should live in `~/.zelligent`.

## 23. Layout source order

Q: Should the fallback order be exactly:
1. repo-local `.zelligent/layout.kdl`
2. user-level `~/.zelligent/layout.kdl`
3. hard error if neither exists

A: Yes.

## 24. Who creates the canonical default layout?

Q: Should `zelligent doctor` create or refresh the canonical user-level layout
at `~/.zelligent/layout.kdl` so the required default exists?

A: Yes, create it if missing. Do not override what people have.

Later clarification:

- Only create it if it does not exist.

## 25. Invalid repo-local layout

Q: If a repo-local `.zelligent/layout.kdl` exists but is invalid under the new
contract, should zelligent hard-fail with a clear error, or ignore it and fall
back to `~/.zelligent/layout.kdl`?

A: Hard-fail with a clear error.

## 26. Invalid user-level layout

Q: Should the same hard-fail rule apply to the user-level
`~/.zelligent/layout.kdl` too?

A: Yes. In that case, tell the user to run `doctor`.

## 27. Manual-tab body limitation

Q: If Zellij only lets us guarantee the sidebar frame for manual tabs but not
the full inner body, should that still satisfy the invariant?

A: Yes, that is fine.

Resulting invariant:

- Manual tabs in a zelligent-managed session must always inherit the zelligent
  sidebar frame.
- Matching the full body is desirable but not required if Zellij does not allow
  full control.

## 28. Default shipped layout location

Q: Where should the shipped canonical default layout live relative to Homebrew
packaging?

A: Look at the Homebrew folder because that determines where Homebrew installs
shared assets.

Resolved from the formula:

- Homebrew installs shared assets in `share/zelligent`.
- Therefore the shipped default layout should live alongside the plugin there.

## 29. Dev and source install layout asset

Q: Should the dev and source install path mirror that same structure too,
meaning `dev-install.sh` should install the default layout next to the dev
plugin?

A: Yes.

## 30. Doctor comparison semantics

Q: When `doctor` checks whether `~/.zelligent/layout.kdl` matches the shipped
default, should that be a strict byte-for-byte comparison, or ignore harmless
differences like comments and whitespace?

A: Byte-by-byte.

## 31. Doctor refresh message

Q: Should the informational `doctor` message for a non-default layout give a
copy command that overwrites the file directly, or a safer command that writes
to a temp path?

A: Overwrite the file directly.

## 32. Placeholder validation beyond sidebar

Q: Should runtime validation enforce only `{{zelligent_sidebar}}` exactly once,
or also reject broken cases like `{{cwd}}` and `{{agent_cmd}}` appearing zero
times in the default layout?

A: Later.

## 33. KDL-shape validation

Q: Should runtime validation also verify that the layout still contains a
syntactically obvious `layout { ... }` wrapper, or avoid KDL-shape validation
beyond placeholder rules?

A: Later.

## 34. Shared rendering path

Q: Should `spawn` and plain `zelligent` use the exact same layout-resolution
function, with only the placeholder values differing by context, or
intentionally keep a separate path?

A: Yes, use one shared rendering path.

Clarification:

- The same function should choose the layout source, validate it, and
  substitute placeholders.
- Only the placeholder values differ by context:
  - plain `zelligent`: `cwd = repo root`, `agent_cmd = $SHELL`
  - `spawn`: `cwd = worktree path`, `agent_cmd = requested command or $SHELL`

## 35. Session startup must configure future manual tabs

Q: When plain `zelligent` creates a new session, do you want it to also
configure the session so future manual tabs inherit the zelligent sidebar
frame?

A: Yes, that has to be a rule, otherwise "always have the sidebar" is broken.

Guaranteed invariant:

- When plain `zelligent` creates a new session, it must configure that session
  so future manually created tabs inherit the zelligent sidebar frame.

## 36. Documentation obligation

Q: Should the docs explicitly state that `.zelligent/layout.kdl` is now a full
layout file with a required `{{zelligent_sidebar}}` placeholder, and that
`{{cwd}}` resolves differently by context?

A: Yes, of course. Harness engineering.

## 37. `init`

Q: Do we even still need `zelligent init`?

A: Yes. Keep it because it creates the per-repo startup and teardown shell
scripts.

Clarification:

- `doctor` is global setup.
- `init` remains for repo-local `setup.sh` and `teardown.sh` stubs.

## 38. `init` and layout scaffolding

Q: Should `init` also create a repo-local `.zelligent/layout.kdl` stub in the
new placeholder format?

A: No KDL. `init` should not create a layout file.

## 39. Doctor and existing layout file

Q: Should `doctor` only create `~/.zelligent/layout.kdl` if it does not exist,
or also refresh and update an existing file?

A: Only if it does not exist.

## 40. Doctor validation scope

Q: Should runtime validation of layout files happen on every relevant command,
or only when the layout is actually being used?

A: Only when the layout is being used.

Additional `doctor` rule:

- `doctor` should only check whether `~/.zelligent/layout.kdl` is the same as
  the current default.
- If not, it should print a message explaining how to overwrite it with a copy
  of the default.

## 41. Doctor scope: which layout file?

Q: When `doctor` says "the current layout," does it mean only
`~/.zelligent/layout.kdl`, or also repo-local `.zelligent/layout.kdl`?

A: Only `~/.zelligent/layout.kdl`.

## 42. Doctor behavior on non-default layout

Q: When `doctor` notices that `~/.zelligent/layout.kdl` differs from the shipped
default, should it print only an informational message, or offer or perform an
explicit refresh action?

A: Only an informational message.

The message should include the exact overwrite command.

## 43. Placeholder substitution style

Q: Should `{{cwd}}` and `{{agent_cmd}}` be pure text substitution that can
appear anywhere in the file, including inside quoted strings and `args`?

A: Yes. Option A: simple global text substitution.

## 44. Sidebar placeholder substitution style

Q: Should `{{zelligent_sidebar}}` also be pure text substitution?

A: Yes.

## 45. Unknown placeholders

Q: If a layout contains an unknown placeholder like `{{foo}}`, should zelligent
hard-fail or leave it untouched as literal text?

A: Leave it untouched. Whatever Zellij would do is fine.

## 46. Doctor and existing invalid layout

Q: Should `doctor` validate an existing `~/.zelligent/layout.kdl`, or only
create it if missing and otherwise leave it alone until runtime?

Initial answer: only create it if missing.

Later refined doctor contract:

- `doctor` should compare `~/.zelligent/layout.kdl` to the shipped default.
- If different, print an informational overwrite command.
- Runtime validation still happens when the layout is actually used.

## 47. Status bar in shipped default

Q: Should the canonical default `~/.zelligent/layout.kdl` that `doctor` creates
include the normal Zellij status bar by default, or omit it?

A: Include it.

## 48. Sidebar placeholder cardinality

Q: Should `{{zelligent_sidebar}}` be required exactly once, or merely at least
once?

A: Exactly once.

## 49. Final "enough questions" checkpoint

Q: Are there any other high-priority product questions?

A: No more high-priority product questions remained. The remaining open items
were implementation and documentation details, plus lower-priority validation
rules left for later.
