# openfeedback

Human-in-the-loop decision gate CLI for AI agents.

When an AI agent (Claude Code, Kiro, or any automation script) reaches a decision point that requires human approval, `openfeedback` sends a rich message to your IM (Telegram) with **Approve / Reject** buttons and waits for a response.

```
Agent → openfeedback send → Telegram → Human clicks Approve/Reject
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
  "request_title": "Deploy hotfix to staging?"
}
```

The `feedback` field is populated when:
- User replies with text after clicking **Reject** (within the feedback window)
- User replies to the original request message (treated as approval with feedback)

## Configuration

`~/.config/openfeedback/config.toml`:

```toml
default_provider = "telegram"
default_timeout = 3600
# Seconds to wait for reject feedback (0 = skip)
reject_feedback_timeout = 60
# locale: "en" (default), "zh-CN", "zh-TW"
locale = "en"

[telegram]
bot_token = "YOUR_BOT_TOKEN"
chat_id = 0
# Only these user IDs can approve/reject. Empty = allow all.
trusted_user_ids = []

[logging]
# audit_file = "~/.local/share/openfeedback/audit.jsonl"
```

| Option | Default | Description |
|--------|---------|-------------|
| `default_timeout` | 3600 | Seconds to wait for approve/reject before timing out |
| `reject_feedback_timeout` | 60 | Seconds to wait for rejection reason (0 = skip) |
| `locale` | `"en"` | UI language: `"en"`, `"zh-CN"`, `"zh-TW"` |
| `trusted_user_ids` | `[]` | Telegram user IDs allowed to respond (empty = all) |

### Telegram Setup

1. Create a bot via [@BotFather](https://t.me/BotFather)
2. Send a message to your bot
3. Get your `chat_id`:
   ```bash
   curl -s "https://api.telegram.org/bot<TOKEN>/getUpdates" | jq '.result[0].message.chat.id'
   ```
4. Add `bot_token`, `chat_id`, and your user ID to `trusted_user_ids`

## Features

- **Single binary** — no runtime dependencies, no server needed
- **Blocking CLI** — sends message, polls for response, exits with result
- **Reject feedback** — after rejection, prompts for a reason (configurable timeout)
- **Timeout cleanup** — removes stale buttons and sends a notice on timeout
- **Trusted users** — whitelist who can approve/reject
- **Audit log** — every decision recorded to JSONL
- **i18n** — English, Simplified Chinese, Traditional Chinese
- **Provider trait** — extensible for Slack, Discord, etc.

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
