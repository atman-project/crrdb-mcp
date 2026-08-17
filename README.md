# crrdb MCP

A MCP (Model Context Protocol) for [crrdb](https://github.com/atman-project/crrdb).

By connecting this MCP to your AI agent, you can store data to your local crrdb, and retrieve/analyze it.

No need to write SQL. Just chat. Your data lives in your local database.

## Usage

Build the MCP server:
```bash
cargo build --release
```

Connect to the MCP server from your AI agent. For example, you can configure Claude:
```bash
claude mcp add crrdb -- $PWD/target/release/crrdb-mcp
claude mcp list
```

Then, open your AI agent and start chatting. Of course, you can access crrdb directly using crrdb CLI
to see if your data has been stored correctly.
