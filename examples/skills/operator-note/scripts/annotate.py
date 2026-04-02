import json
import sys


def main() -> None:
    payload = json.load(sys.stdin)
    rendered = payload.get("rendered_prompt", "").strip()
    sandbox = payload.get("sandbox", {})
    env_id = sandbox.get("env_id", "unknown-env")
    content = f"{rendered}\n\nhelper_script=annotate.py\nsandbox_env={env_id}".strip()
    print(
        json.dumps(
            {
                "content": content,
                "output_mode": "json",
                "structured": {
                    "script": "annotate.py",
                    "sandbox_env": env_id,
                },
            }
        )
    )


if __name__ == "__main__":
    main()
