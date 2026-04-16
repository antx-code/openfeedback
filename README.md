# openfeedback

Human-in-the-loop decision gate CLI for AI agents.

When an AI agent (Claude Code, Kiro, or any automation script) reaches a decision point that requires human approval, `openfeedback` sends a rich message to your IM (**Telegram** or **Discord**) with **Approve / Reject** buttons and waits for a response.

It also supports **failover**: if the primary channel doesn't answer within a configurable window, the pending request is cleaned up (buttons removed + "escalated" notice posted) and the request is re-sent on a secondary channel — so a silent primary never blocks your agent.

```
Agent → openfeedback send → Telegram / Discord → Human clicks Approve/Reject
                          ← exit code + JSON ←
```

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

## Quick Start

```bash
# 1. Initialize config
openfeedback init
# Edit ~/.config/openfeedback/config.toml (or ~/Library/Application Support/openfeedback/config.toml on macOS)

# 2. Send a request
openfeedback send --title "Deploy to prod?" --body "Release v2.0 with breaking changes"

# 3. Or use a markdown file
openfeedback send --title "Review Plan" --body-file /tmp/plan.md --timeout 300
```

## How It Works

1. **Send** — bot posts a message with ✅ Approve / ❌ Reject buttons
2. **Approve** — exits immediately with code `0`
3. **Reject** — bot asks for a reason; user can reply with any text message (or ignore to skip), then exits with code `1`
4. **Timeout** — buttons are removed, a timeout notice is sent, exits with code `2`

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Approved |
| 1 | Rejected |
| 2 | Timeout |

## Output

JSON to stdout for agent consumption:

```json
{
  "decision": "rejected",
  "user": "@username",
  "user_id": 123456789,
  "feedback": "Not ready, need more tests",
  "timestamp": "2026-03-09T13:48:29Z",
  "request_title": "Deploy hotfix to staging?",
  "provider": "telegram"
}
```

The `feedback` field is populated when:
- User replies with text after clicking **Reject** (within the feedback window)
- User replies to the original request message (treated as approval with feedback)

The `provider` field identifies which channel produced the decision. If a failover occurred, an `escalated_from` field is also present:

```json
{
  "decision": "approved",
  "provider": "discord",
  "escalated_from": "telegram",
  "...": "..."
}
```

## Configuration

`~/.config/openfeedback/config.toml`:

```toml
default_provider = "telegram"    # "telegram" or "discord"
default_timeout = 3600
# Seconds to wait for reject feedback (0 = skip)
reject_feedback_timeout = 60
# locale: "en" (default), "zh-CN", "zh-TW"
locale = "en"

# --- Optional failover ---
# If set, when the primary times out the CLI cleans up (removes buttons + posts an
# "escalated" notice) and re-sends on this secondary provider.
# failover_provider = "discord"
# escalate_after_secs = 1800     # default: half of default_timeout

[telegram]
bot_token = "YOUR_BOT_TOKEN"
chat_id = 0
# Only these user IDs can approve/reject. Empty = allow all.
trusted_user_ids = []

# [discord]
# bot_token = "YOUR_BOT_TOKEN"
# application_id = "YOUR_APPLICATION_ID"
# channel_id = "YOUR_CHANNEL_ID"
# trusted_user_ids = []            # Discord snowflakes as strings

[logging]
# audit_file = "~/.local/share/openfeedback/audit.jsonl"
```

| Option | Default | Description |
|--------|---------|-------------|
| `default_provider` | `"telegram"` | Which provider receives requests: `"telegram"` or `"discord"` |
| `default_timeout` | 3600 | Total seconds to wait for approve/reject before timing out |
| `reject_feedback_timeout` | 60 | Seconds to wait for rejection reason (0 = skip) |
| `locale` | `"en"` | UI language: `"en"`, `"zh-CN"`, `"zh-TW"` |
| `failover_provider` | *(none)* | Optional secondary provider, activated when primary doesn't answer in time |
| `escalate_after_secs` | `default_timeout / 2` | How long to wait on the primary before escalating |
| `trusted_user_ids` | `[]` | Per-provider whitelist (empty = allow all) |

### Telegram Setup

1. Create a bot via [@BotFather](https://t.me/BotFather)
2. Send a message to your bot
3. Get your `chat_id`:
   ```bash
   curl -s "https://api.telegram.org/bot<TOKEN>/getUpdates" | jq '.result[0].message.chat.id'
   ```
4. Add `bot_token`, `chat_id`, and your user ID to `trusted_user_ids`

### Discord Setup

1. Create an application at <https://discord.com/developers/applications>
2. Under **Bot**, reset/copy the **Bot Token** → `discord.bot_token`
3. Copy the **Application ID** from *General Information* → `discord.application_id`
4. Under **Bot → Privileged Gateway Intents**, enable **MESSAGE CONTENT INTENT**
   (needed so the bot can read reply-text feedback)
5. Under **OAuth2 → URL Generator**, pick scopes `bot` + permissions
   `Send Messages`, `Read Message History` (and `Use Slash Commands` if you want),
   then invite the bot to your server
6. Right-click the target channel → **Copy Channel ID** → `discord.channel_id`
   (turn on Developer Mode in Discord settings first)
7. Optionally add your own Discord user ID(s) to `discord.trusted_user_ids`

### Failover (Telegram + Discord)

Configure both providers, then:

```toml
default_provider = "telegram"
failover_provider = "discord"
escalate_after_secs = 1800       # try Telegram for 30 min, then Discord
default_timeout = 3600           # total budget (Telegram + Discord)
```

Behavior:

- **Approve / Reject on Telegram** — returns immediately, Discord is never touched.
- **No response within 30 min on Telegram** — Telegram buttons are removed, an "escalated" notice is posted in Telegram, and a fresh request with buttons is posted to Discord (remaining 30 min budget).
- **No response on Discord either** — Discord buttons are removed, timeout notice posted, CLI exits `2`.

At any moment only **one** channel has live buttons, so rejection reasons are never split between channels.

### CLI flags

```bash
# Default: follow config (with failover if configured)
openfeedback send --title "..." --body "..."

# Override to a single provider, disable failover entirely
openfeedback send --provider discord --title "..." --body "..."

# Override total timeout
openfeedback send --title "..." --body "..." --timeout 600
```

## Features

- **Single binary** — no runtime dependencies, no server needed
- **Blocking CLI** — sends message, waits for response, exits with result
- **Telegram + Discord** — native buttons on both (Telegram long-polling, Discord Gateway)
- **Failover** — automatic cleanup + handoff to a secondary provider on primary timeout
- **Reject feedback** — after rejection, prompts for a reason (configurable timeout)
- **Timeout cleanup** — removes stale buttons and sends a notice on timeout
- **Trusted users** — per-provider whitelist of who can approve/reject
- **Audit log** — every decision recorded to JSONL with `provider` + `escalated_from`
- **i18n** — English, Simplified Chinese, Traditional Chinese

## Use with AI Agents

### Shell script

```bash
result=$(openfeedback send --title "Proceed with PR?" --body-file /tmp/plan.md)
exit_code=$?

if [ $exit_code -eq 0 ]; then
  echo "Approved, continuing..."
elif [ $exit_code -eq 1 ]; then
  feedback=$(echo "$result" | jq -r '.feedback // empty')
  echo "Rejected: $feedback"
else
  echo "Timeout, aborting"
fi
```

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
