# Copyright 2026 ZyvorAI Labs Private Limited
# SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

"""Optional CLI entry point when running without maturin develop."""

import sys


def main() -> None:
    print(
        "Use the Rust CLI: cargo run -p zyvor-janus-cli -- run --config <path>",
        file=sys.stderr,
    )
    print(
        "Or from Python: import zyvor_janus; zyvor_janus.run_from_config('<path>')",
        file=sys.stderr,
    )
    sys.exit(1)


if __name__ == "__main__":
    main()
