# Controlling Pakespec from other programs

Two front ends share one layer, `src/automation.rs` (`Session`): it turns a JSON request into a board operation and returns a JSON reply. The board is opened on the first command that needs it and reopened after a board error.

## Command line (`pakespec-cli`)

| Flags | Result |
|---|---|
| `--list-ports --json` | Serial ports; `board_candidate` and `suggested` mark the likely board |
| `--status` | Session state; does not touch the board |
| `--json [--start HZ --stop HZ --points N]` | One sweep as JSON (arrays `freq_hz`, `raw`, `dbm`, plus the highest point as `peak`) |
| `--measure HZ` | One reading at a fixed frequency; the output is stopped afterwards |
| `--generator HZ` / `--generator-off` | Set or stop the tracking-source frequency |
| `--serve` | Keep the board open; one JSON request per input line, one reply per line; `{"cmd":"quit"}` ends |

Without `--json` a sweep is printed as CSV. `--port PATH`, `--simulate` and `--calibration FILE` apply to all of them. `dbm` is `null` where no calibration covers the reading.

Replies are `{"ok": true, ...}` or `{"ok": false, "error": {"code": "...", "message": "..."}}`; a request `id` is copied to the reply.

Exit codes: 0 success, 1 other failure, 2 bad command line or request (`invalid_request`, `invalid_parameter`, `unknown_command`, `not_allowed`), 3 board not found or cannot be opened (`no_board`, `open_failed`), 4 board failed while in use (`board_timeout`, `board_io`, `board_protocol`).

Requests in `--serve` mode: `{"cmd": "sweep", "start": 35000000, "stop": 1000000000, "points": 101}`, `{"cmd": "measure", "freq_hz": 100000000}`, `{"cmd": "generator", "freq_hz": ...}`, `{"cmd": "generator_off"}`, `{"cmd": "status"}`, `{"cmd": "list_ports"}`.

## MCP server (`pakespec-mcp`)

JSON-RPC 2.0 on standard input and output, one message per line, no extra library. Tools: `list_ports`, `status`, `sweep`, `measure`, and `generator`, `generator_off`. Tool results are the same JSON as above, as text; failures set `isError`.

Options: `--port PATH`, `--simulate`, `--calibration FILE` (default: the calibration file next to the settings file, when it exists), `--no-generator` (hides the two generator tools).

Example client configuration:

```json
{"mcpServers": {"pakespec": {"command": "pakespec-mcp", "args": ["--port", "/dev/ttyUSB0"]}}}
```

## What a model can and cannot do

- It can sweep and read the detector; both leave the tracking source off unless the S1 button on the board is on.
- It can set the tracking-source frequency, but not switch the output: only the S1 button does, and the software cannot read it. The tool description tells the model to ask what is connected to RF OUT first.
- It cannot change the calibration, write files or send anything to the board other than sweep, read and frequency commands.
- Use `--no-generator` when the model should only observe.
- The board input takes at most +5 dBm; the software cannot check what is connected.
