# Security Policy

## Supported versions

| Version | Supported |
|---------|-----------|
| 0.1.x   | Security patches |
| < 0.1   | Not supported |

## Reporting a vulnerability

These plugins handle token swaps, treasury operations, and payment requests. A vulnerability could result in financial loss.

Do not open a public GitHub issue for security vulnerabilities. Send an email to the repository maintainers. If you do not receive a response within 48 hours, open a private security advisory on GitHub:

1. Go to `https://github.com/Idle0x/squads-defi-suite/security/advisories`
2. Click "New draft security advisory"
3. Include reproduction steps, potential impact, and a suggested fix if available

### Reportable issues

- **Guardrail bypass**: A swap that should have been blocked by config-enforced limits (mint allowlist, slippage, notional cap, daily cap) was allowed.
- **Recipient redirection**: A Solana Pay URL was built with a recipient other than the configured address.
- **Config leak**: `__config` values were included in plugin output, logs, or error messages.
- **Unsigned transaction with incorrect parameters**: The meta-transaction contains unexpected instructions or parameters.
- **RPC credential exposure**: RPC URL or API key leaked in output or logs.

### Response timeline

1. Acknowledgment within 48 hours.
2. Investigation and severity determination.
3. Fix development and patched release.
4. Advisory publication after the fix is released.

### Bug bounty

This project does not currently operate a bug bounty program. Reporters will be credited in advisory publications and release notes.
