#!/usr/bin/env python3

import json
import sys
from pathlib import Path


def send(response):
    sys.stdout.write(json.dumps(response) + "\n")
    sys.stdout.flush()


def filesystem_tools():
    return {
        "tools": [
            {
                "name": "read_file",
                "description": "Read a UTF-8 text file from disk via MCP",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"},
                    },
                    "required": ["path"],
                },
            }
        ]
    }


def filesystem_call(params):
    arguments = params.get("arguments") or {}
    path = arguments.get("path")

    if not path:
        return {
            "content": [{"type": "text", "text": "missing required argument: path"}],
            "structuredContent": {"path": None},
            "isError": True,
        }

    try:
        content = Path(path).read_text(encoding="utf-8")
    except Exception as exc:
        return {
            "content": [{"type": "text", "text": str(exc)}],
            "structuredContent": {"path": path},
            "isError": True,
        }

    return {
        "content": [{"type": "text", "text": content}],
        "structuredContent": {"path": path, "content": content},
        "isError": False,
    }


def main():
    mode = sys.argv[1] if len(sys.argv) > 1 else "filesystem"

    if mode == "crash":
        return 1

    for raw_line in sys.stdin:
        line = raw_line.strip()
        if not line:
            continue

        if mode == "invalid":
            sys.stdout.write("{invalid json}\n")
            sys.stdout.flush()
            continue

        request = json.loads(line)
        method = request.get("method")
        params = request.get("params") or {}
        response = {
            "jsonrpc": "2.0",
            "id": request.get("id"),
        }

        if mode != "filesystem":
            response["error"] = {"message": f"unsupported mock mode: {mode}"}
            send(response)
            continue

        if method == "tools/list":
            response["result"] = filesystem_tools()
            send(response)
            continue

        if method == "tools/call":
            if params.get("name") != "read_file":
                response["error"] = {"message": f"unknown tool: {params.get('name')}"}
            else:
                response["result"] = filesystem_call(params)
            send(response)
            continue

        response["error"] = {"message": f"unknown method: {method}"}
        send(response)

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
