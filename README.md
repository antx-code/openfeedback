# openfeedback

Human-in-the-loop decision gate CLI for AI agents.

When an AI agent (Claude Code, Kiro, or any automation script) reaches a decision point that requires human approval, `openfeedback` sends a rich message to your IM (Telegram) with **Approve / Reject** buttons and waits for a response.

```
Agent → openfeedback send → Telegram → Human clicks Approve/Reject
                          ← exit code + JSON ←
```

## Install

```bash
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
  "decision": "approved",
  "user": "@username",
  "user_id": 123456789,
  "feedback": null,
  "timestamp": "2026-03-09T09:25:27Z",
  "request_title": "Deploy to prod?"
}
```

## Configuration

`~/.config/openfeedback/config.toml`:

```toml
default_provider = "telegram"
default_timeout = 3600
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

### Telegram Setup

1. Create a bot via [@BotFather](https://t.me/BotFather)
2. Send a message to your bot
3. Get your `chat_id`:
   ```bash
   curl -s "https://api.telegram.org/bot<TOKEN>/getUpdates" | jq '.result[0].message.chat.id'
   ```
4. Add `bot_token`, `chat_id`, and your user ID to `trusted_user_ids`

## Features

- **Single binary** - no runtime dependencies, no server needed
- **Blocking CLI** - sends message, polls for response, exits with result
- **Trusted users** - whitelist who can approve/reject
- **Audit log** - every decision recorded to JSONL
- **i18n** - English, Simplified Chinese, Traditional Chinese
- **Provider trait** - extensible for Slack, Discord, etc.

## Use with AI Agents

```bash
# In a shell script or agent hook
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

## License

[MIT](LICENSE)
