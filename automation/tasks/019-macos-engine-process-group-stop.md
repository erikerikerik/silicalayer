# Stop the managed ComfyUI and everything it started, gracefully, on macOS

Correctness fix for V1 item 6 (the private engine shuts down cleanly and only when owned). On macOS, `kill_tree` in `crates/aigen/src/launch.rs` (about line 833) only calls `child.kill()`: SIGKILL to the Python process alone. ComfyUI gets no chance to finish writing, and any process it started (PyTorch workers, a helper `python`) can be left running and holding unified memory. Windows already ends the whole tree with `taskkill /T`.

Scope (non-Windows only; leave every `#[cfg(windows)]` path exactly as it is):

- Where the managed engine is spawned (`let mut cmd = Command::new(python);` and `cmd.spawn()`, about lines 807–826): on Unix, start it in its own process group with `std::os::unix::process::CommandExt::process_group(0)`, so a group signal reaches its children and never the app.
- `fn kill_tree` (about line 833): on Unix, send SIGTERM to the child's process group, poll `child.try_wait()` for up to 10 seconds (short sleeps), then send SIGKILL to the group if anything is still alive, and finally `wait()` to reap. Send group signals with `/bin/kill -TERM -- -<pgid>` and `/bin/kill -KILL -- -<pgid>` through `std::process::Command` (stdin/stdout/stderr null). Do not add a crate dependency.
- `stop()` (about line 906) keeps its contract: it only acts on a child this process started (`self.child()`), and a ComfyUI the app found running is never signalled. Do not change `is_running` or the "a running server is used and never stopped" behaviour.

Tests (Unix only, `#[cfg(unix)]`, in this file's existing test module; each must fail without the fix):

- Spawn, through the same spawn path or a small helper it uses, a stand-in `sh -c 'trap "exit 0" TERM; sleep 60 & wait'` whose child `sleep` is recorded (for example by writing `$!` to a temp file). After `kill_tree`, both the shell and the `sleep` are gone (`/bin/kill -0 <pid>` fails for each) within 12 seconds.
- A stand-in that ignores SIGTERM (`trap "" TERM; sleep 60`) is ended by the SIGKILL fallback, and `kill_tree` returns within about 12 seconds.
- The process group of the spawned stand-in differs from the test process's own group (`ps -o pgid= -p <pid>`).

Edit only `crates/aigen/src/launch.rs`. Do not read `target/` or anything outside the repository.

Exit evidence: the full supervisor gate passes.
