# openfeedback

Human-in-the-loop decision gate CLI for AI agents.

When an AI agent (Claude Code, Kiro, or any automation script) reaches a decision
point that requires human approval, `openfeedback` sends a rich message to your
IM (**Telegram** or **Discord**) with **Approve / Reject** buttons and blocks until
a response (or timeout) is received. The outcome is emitted as a JSON blob on
stdout and signaled via the process exit code.

It also supports **failover**: if the primary channel is silent past a
configurable window, the CLI cleans up (removes stale buttons, posts an
"escalated" notice) and re-sends the request on a secondary channel, so a
silent primary never indefinitely blocks your agent.

```
Agent → openfeedback send → Telegram / Discord → Human clicks Approve/Reject
                          ← exit code + JSON ←
```

---

## Agent integration contract

> This section is optimized for automated consumers. If you are an AI agent or
> building one, everything you need to integrate is below. Scroll past for the
> human-oriented sections.

### Invocation

```
openfeedback send --title <TITLE> [--body <TEXT> | --body-file <PATH>]
                  [--timeout <SECS>] [--provider telegram|discord]
```

- `--title` (required): short headline shown in bold in the IM message.
- `--body` / `--body-file`: long-form Markdown body. If both are omitted, the
  body is empty.
- `--timeout`: overrides `default_timeout` from config for this one call.
- `--provider`: forces a single provider and **disables failover** for this
  call, regardless of config.

### Exit codes

| Code | Meaning |
|------|---------|
| `0`  | Approved |
| `1`  | Rejected |
| `2`  | Timeout (no human response before the deadline) |
| non-`0/1/2` | Error (config invalid, network failure, etc.) — see stderr |

### stdout JSON schema

Every invocation that reaches a terminal decision prints exactly one JSON
object to stdout:

```json
{
  "decision": "approved" | "rejected" | "timeout",
  "user": "@username or Display Name",
  "user_id": 123456789,
  "feedback": "optional reject reason or reply text",
  "timestamp": "2026-04-16T23:40:27.107612Z",
  "request_title": "original --title value",
  "provider": "telegram" | "discord",
  "escalated_from": "telegram" | "discord"
}
```

Field semantics:

| Field | Type | Always present | Notes |
|-------|------|---|---|
| `decision` | string | yes | One of `approved`, `rejected`, `timeout`. |
| `user` | string | yes | Human-readable identifier; empty string on `timeout`. |
| `user_id` | integer | yes | Platform-native user ID; `0` on `timeout`. |
| `feedback` | string \| null | yes | Set when user typed a reject reason, or replied to the original request (treated as approval with feedback). `null` otherwise. |
| `timestamp` | RFC3339 UTC string | yes | When the decision was captured. |
| `request_title` | string | yes | Echo of `--title`. |
| `provider` | string | yes (as of v0.3.0) | The provider that actually produced the decision. |
| `escalated_from` | string | only if failover fired | Name of the primary that timed out before this provider took over. Absent (omitted from JSON) when no failover occurred. |

Reject-with-feedback semantics: a user can (a) click **Reject** and then type a
reason within `reject_feedback_timeout` seconds, or (b) reply to the original
message with plain text. Case (a) yields `{"decision":"rejected","feedback":"..."}`.
Case (b) yields `{"decision":"approved","feedback":"..."}` — a "yes, with a note"
pattern that works identically on Telegram and Discord.

### Minimal agent wrapper (Bash)

```bash
result=$(openfeedback send --title "Proceed with PR?" --body-file /tmp/plan.md)
case $? in
  0) feedback=$(jq -r '.feedback // empty' <<<"$result")
     echo "APPROVED ${feedback:+with note: $feedback}" ;;
  1) echo "REJECTED: $(jq -r '.feedback // "no reason given"' <<<"$result")" ;;
  2) echo "TIMEOUT — human did not respond in time" ;;
  *) echo "ERROR"; exit 1 ;;
esac
```

### Minimal agent wrapper (Python)

```python
import json, subprocess

p = subprocess.run(
    ["openfeedback", "send", "--title", "Proceed with PR?",
     "--body-file", "/tmp/plan.md", "--timeout", "600"],
    capture_output=True, text=True,
)
result = json.loads(p.stdout) if p.stdout else {}
match p.returncode:
    case 0: print("APPROVED", result.get("feedback") or "")
    case 1: print("REJECTED:", result.get("feedback") or "no reason")
    case 2: print("TIMEOUT")
    case _: print("ERROR:", p.stderr); raise SystemExit(1)
```

---

## Install

```bash
cargo install --git https://github.com/antx-code/openfeedback.git
```

Or build from source:

```bash
git clone https://github.com/antx-code/openfeedback.git
cd openfeedback
cargo install --path .
```

Requirements: Rust 1.80+ (edition 2024). On Linux you may need `pkg-config`
and `libssl-dev` for `reqwest`'s TLS backend.

## Quick start

```bash
# 1. Initialize the config file (writes a template you then fill in)
openfeedback init

# On macOS: ~/Library/Application Support/openfeedback/config.toml
# On Linux: ~/.config/openfeedback/config.toml

# 2. Send a request
openfeedback send --title "Deploy to prod?" --body "Release v2.0 with breaking changes"

# 3. Or use a markdown file
openfeedback send --title "Review Plan" --body-file /tmp/plan.md --timeout 300
```

## How it works

1. **Send** — bot posts a message with ✅ Approve / ❌ Reject buttons.
2. **Approve** — exits immediately with code `0`.
3. **Reject** — bot asks for a reason; user can reply with any text message
   (or ignore to skip), then exits with code `1`.
4. **Timeout** — buttons are removed, a timeout notice is sent, exits with code `2`.

## Configuration

Full reference. Config file location (created by `openfeedback init`):

- macOS: `~/Library/Application Support/openfeedback/config.toml`
- Linux: `~/.config/openfeedback/config.toml`

```toml
# --- Top-level ---
default_provider = "telegram"    # "telegram" or "discord"
default_timeout = 3600           # Total seconds to wait for a decision
reject_feedback_timeout = 60     # Seconds to wait for reject reason (0 = skip)
locale = "en"                    # "en" | "zh-CN" | "zh-TW"

# --- Optional failover ---
# When set, if `default_provider` times out without a decision, the CLI
# removes its buttons, posts an "escalated" notice, and re-sends the request
# on `failover_provider` with the remaining budget.
# failover_provider = "discord"
# escalate_after_secs = 1800     # default: default_timeout / 2
                                 # MUST be < default_timeout

# --- Telegram (required if referenced by default_provider or failover_provider) ---
[telegram]
bot_token = "YOUR_BOT_TOKEN"
chat_id = 0                      # int64, can be negative for groups
trusted_user_ids = []            # Empty = allow anyone in the chat

# --- Discord (required if referenced by default_provider or failover_provider) ---
# [discord]
# bot_token = "YOUR_BOT_TOKEN"
# application_id = "YOUR_APPLICATION_ID"
# channel_id = "YOUR_CHANNEL_ID"   # string (snowflake, to avoid JSON precision issues)
# trusted_user_ids = []            # strings (snowflakes)

[logging]
# audit_file = "~/.local/share/openfeedback/audit.jsonl"
```

### Option reference

| Option | Default | Description |
|--------|---------|-------------|
| `default_provider` | `"telegram"` | Which provider receives requests: `"telegram"` or `"discord"` |
| `default_timeout` | `3600` | Total seconds budget across primary + failover |
| `reject_feedback_timeout` | `60` | Seconds to wait for a reject reason after the button click (`0` = skip the prompt) |
| `locale` | `"en"` | UI language: `"en"`, `"zh-CN"`, `"zh-TW"` |
| `failover_provider` | *(none)* | Optional secondary provider; must differ from `default_provider` |
| `escalate_after_secs` | `default_timeout / 2` | How long to wait on the primary before escalating. **Must be `< default_timeout`** when `failover_provider` is set (validated at load time) |
| `telegram.bot_token` | *(required)* | From [@BotFather](https://t.me/BotFather) |
| `telegram.chat_id` | *(required)* | Int64; can be negative for groups |
| `telegram.trusted_user_ids` | `[]` | Empty = allow anyone in the chat |
| `discord.bot_token` | *(required)* | From Discord Developer Portal → your app → Bot → Reset Token |
| `discord.application_id` | *(required)* | From your app → General Information |
| `discord.channel_id` | *(required)* | Right-click a channel → Copy Channel ID (enable Developer Mode first). Stored as a string to avoid JSON precision loss on the 64-bit snowflake. |
| `discord.trusted_user_ids` | `[]` | Strings (snowflakes); empty = allow anyone who can see the channel |

### Telegram setup

1. Create a bot via [@BotFather](https://t.me/BotFather).
2. Send any message to your bot (this opens a DM so the bot can post).
3. Get your `chat_id`:
   ```bash
   curl -s "https://api.telegram.org/bot<TOKEN>/getUpdates" | jq '.result[0].message.chat.id'
   ```
4. Fill in `telegram.bot_token`, `telegram.chat_id`, and your own user ID in
   `telegram.trusted_user_ids`.

### Discord setup

1. Create an application at <https://discord.com/developers/applications>.
2. Under **Bot**, reset/copy the **Bot Token** → `discord.bot_token`.
3. Copy the **Application ID** from *General Information* → `discord.application_id`.
4. Under **Bot → Privileged Gateway Intents**, enable **MESSAGE CONTENT INTENT**
   (needed so the bot can read reply-text feedback).
5. Under **OAuth2 → URL Generator**, pick scope `bot` and permissions
   `Send Messages`, `Read Message History`, then open the generated URL and
   invite the bot to your server.
6. Right-click the target channel → **Copy Channel ID** → `discord.channel_id`
   (enable Developer Mode under **User Settings → Advanced** first).
7. Copy your own user ID (right-click avatar → Copy User ID) into
   `discord.trusted_user_ids`.

### Failover (Telegram + Discord)

Configure both providers, then:

```toml
default_provider   = "telegram"
failover_provider  = "discord"
default_timeout    = 3600        # total budget (primary + failover)
escalate_after_secs = 1800       # try primary for 30 min, then failover
```

Exactly one channel has live buttons at any moment, so rejection reasons are
never split across channels.

Decision matrix:

| Scenario | Outcome | `provider` | `escalated_from` |
|---|---|---|---|
| Human clicks on primary within `escalate_after_secs` | Decision returned | primary | *(omitted)* |
| Primary silent past `escalate_after_secs`, human clicks on secondary | Primary gets cleanup notice + buttons removed; secondary decision returned | secondary | primary |
| Both silent past `default_timeout` | `decision=timeout`, both get cleanup notice | secondary | primary |
| `--provider` CLI flag passed | Only that one provider is used; no failover | that provider | *(omitted)* |

Validation (enforced at config load time):

- `failover_provider` must differ from `default_provider`.
- Both referenced providers must have their `[telegram]` / `[discord]` sections.
- If `escalate_after_secs` is explicitly set, it must be strictly less than
  `default_timeout` (so the secondary has a non-zero budget).

### CLI flags

```bash
# Default: follow config (with failover if configured)
openfeedback send --title "..." --body "..."

# Override to a single provider; disables failover for this call
openfeedback send --provider discord --title "..." --body "..."

# Override total timeout for a single call
openfeedback send --title "..." --body "..." --timeout 600

# Body from a Markdown file
openfeedback send --title "..." --body-file ./plan.md
```

## Upgrading

### From v0.2.x → v0.3.x

v0.3.x introduces **Discord support** and **failover**. All v0.2.x configs and
invocations remain valid; the new features are opt-in.

Required action: **none** if you only use Telegram and don't want failover.

To enable Discord:

1. Cargo install the new version:
   ```bash
   cargo install --git https://github.com/antx-code/openfeedback.git --force
   ```
2. Follow **[Discord setup](#discord-setup)** above to create a bot and get
   credentials.
3. Add the `[discord]` section to your existing config:
   ```toml
   [discord]
   bot_token = "..."
   application_id = "..."
   channel_id = "..."
   trusted_user_ids = ["..."]
   ```
4. Optionally enable failover:
   ```toml
   failover_provider = "discord"
   escalate_after_secs = 1800    # < default_timeout
   ```

New output fields (stdout JSON):

- `provider` — which provider produced the decision (always present as of v0.3.0).
- `escalated_from` — present only when failover fired; names the primary that
  timed out.

New CLI flag:

- `--provider <telegram|discord>` — force a single provider for one call,
  bypassing `default_provider` and any configured failover.

Config validation (new in v0.3.0): when `failover_provider` is set,
`escalate_after_secs` must be strictly less than `default_timeout`, otherwise
config load fails with a clear error. This prevents the silent failure mode
where the secondary would be invoked with a 0-second budget.

Breaking changes: **none**. v0.2.x integrations (parsing `decision`, `user`,
`feedback`, exit codes) continue to work unchanged.

## Features

- **Single binary** — no runtime dependencies, no server needed.
- **Blocking CLI** — sends message, waits, exits with the result.
- **Telegram + Discord** — native buttons on both (Telegram long-polling,
  Discord Gateway WSS).
- **Failover** — automatic cleanup + handoff to a secondary provider on
  primary timeout, with a clear escalation notice in the primary channel.
- **Reject feedback** — after rejection, prompts for a reason; accepts either
  a reply to the prompt or the original message.
- **Reply-as-approval-with-feedback** — replying directly to the request
  message is treated as "approve, with note".
- **Timeout cleanup** — removes stale buttons and posts a notice when the
  deadline is reached.
- **Trusted users** — per-provider whitelist of who can approve/reject.
- **Audit log** — every decision recorded to JSONL with `provider` and
  `escalated_from`.
- **i18n** — English, Simplified Chinese, Traditional Chinese.
- **HTTP(S) proxy** — honors `HTTPS_PROXY`/`HTTP_PROXY` for both REST calls
  (Telegram/Discord) and the Discord Gateway WSS connection (via HTTP CONNECT
  tunneling).

## Examples

### Claude Code hook

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hook": "openfeedback send --title 'Bash command approval' --body-file /tmp/cmd.md --timeout 300"
      }
    ]
  }
}
```

### Cron + non-interactive agent

```bash
# cron checks for new tasks every 5 minutes
*/5 * * * * /path/to/check-and-dispatch.sh

# Inside the script, the agent calls openfeedback at decision gates
claude --print --permission-mode bypassPermissions "Analyze issue and propose a plan"
# Agent reaches gate → openfeedback send → waits for human → continues or stops
```

## License

[MIT](LICENSE)
