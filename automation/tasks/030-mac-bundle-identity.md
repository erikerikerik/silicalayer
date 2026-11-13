# Ship the Mac app as SilicaLayer with its own bundle identifier

Phase 5/6 identity, macOS packaging only. The Mac bundle still carries upstream PhotoCraft's identity: `packaging/macos/Info.plist.in` has `CFBundleName`, `CFBundleDisplayName` and `CFBundleExecutable` = `PhotoCraft` (about lines 40–44) and `CFBundleIdentifier` = `ai.storyteller.photocraft` (about line 46); `packaging/macos/package.sh` builds `PhotoCraft.app` (about lines 42, 118–121, 156), names the DMG `photocraft-$VERSION-macos-$ARCH.dmg` (about line 43) and signs the CLI as `ai.storyteller.photocraft-cli` (about lines 140 and 185). A signed, notarized SilicaLayer release needs its own identifier, and upstream's must not be reused.

Scope:

- `packaging/macos/Info.plist.in`: `CFBundleName`, `CFBundleDisplayName` and `CFBundleExecutable` = `SilicaLayer`; `CFBundleIdentifier` = `io.github.erikerikerik.silicalayer`. Keep the document type UTIs (`ai.storyteller.photocraft.document`, `.ora`, `.kra`, `.pdn`, `.qoi`) unchanged: they describe file formats shared with upstream, as LayerShop kept its PhotoCraft machine names.
- `packaging/macos/package.sh`: build `SilicaLayer.app`, with the executable and icon names following `CFBundleExecutable`/`CFBundleIconFile` as the script already does; name the DMG `silicalayer-$VERSION-macos-$ARCH.dmg` and the CLI zip `silicalayer-cli-$VERSION-macos-$ARCH.zip`; sign the CLI with identifier `io.github.erikerikerik.silicalayer-cli`. Keep the built binary names `photocraft` and `photocraft-cli` inside the build (only the bundle's executable is renamed, as the script copies it). Update the script's header comment.
- `packaging/macos/verify.sh`: whatever it checks by name or identifier follows the new values.
- `packaging/macos/dmg/README.md`: the app name in the layout notes.
- Windows, Linux and web packaging are unchanged.

Checks (the gate does not run packaging; make these hold by reading):

- `grep -n 'ai.storyteller.photocraft"' packaging/macos/Info.plist.in` finds nothing (only the document UTIs keep the prefix).
- `bash -n packaging/macos/package.sh` and `bash -n packaging/macos/verify.sh` pass.
- Add a small test in `xtask` or a shell check only if the repo already has one for packaging; otherwise none.

Edit only files under `packaging/macos/`. Do not add files. Do not read `target/` or anything outside the repository.

Exit evidence: the full supervisor gate passes (it builds the app; packaging is checked by Erik's next local DMG build).
