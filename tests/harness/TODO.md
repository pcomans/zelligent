# Harness TODO

- Update the harness executor to launch the plugin directly with `zellij action launch-or-focus-plugin -f -m file:$HOME/.local/share/zelligent/zelligent-plugin.wasm` and stop depending on `Ctrl-Y`.
- Verify the updated startup flow works end to end in the live tmux harness: launch `zelligent.sh`, dismiss `About Zellij`, launch the plugin directly, then confirm the sidebar is actually visible.
- Add a reusable preflight helper or procedure that fails immediately when the `view` pane cwd is not `/private/tmp/zelligent-test-repo`.
- Add a reusable teardown verification step that checks both process state and `session_info` cache state before the next plan starts.
- Re-run all three harness plans and update them again if any step still assumes behavior that the real product does not provide.
- Decide whether the harness should verify the `Ctrl-Y` keybinding separately in a focused product-level test, instead of mixing that concern into every sidebar harness run.
- Once the startup path is stable, add one plan that exercises plugin resurrection or explicitly documents that resurrection is out of scope because of the known cwd bug.
