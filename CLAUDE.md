# Settings Migrator

Rust workspace: `crates/engine` (library), `crates/cli` (`settings-migrator` binary), `recipes/` (one YAML recipe per app). The full plan is in `Project Spec.md`.

## Rules

- **Never copy secrets.** Passwords, tokens, cookies, session data and private keys must never end up in a bundle. Recipes list only settings files; anything that might hold a secret is excluded, and the engine's denylist blocks known secret files whatever a recipe says.
- **Every restore backs up originals first.** Before overwriting or creating a file, record it in an undo set, so `undo` can put things back exactly as they were.
- **Every command that writes files has a dry-run option** that shows what would happen and writes nothing.
- **One phase at a time.** Only build the phase you were asked for. Note follow-ups for later phases; don't build them.
- **Keep CI green.** Before saying a change is done, run every `run:` step in `.github/workflows/ci.yml` exactly as written, not just fmt, clippy and tests. Code must work on both Windows and macOS.
- **Never commit or push.** The user does that. Say when it's a good moment to commit and suggest a message.
- **Tests never touch real settings.** Use fake files in a temp folder and pass a fake home directory (`PathContext::for_home`, or `--home` on the CLI).
