<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# lib-service

A HypeR component. See [HypeR](https://github.com/HypeR-Proj/HypeR) for
workspace setup and the installed SDK build boundary.

## Workspace tests

Use the [HypeR workspace](https://github.com/HypeR-Proj/HypeR) to resolve dependencies:

```sh
python3 project.py test lib-service
```

Paths in the documentation are relative to the repo-managed workspace root.
Edit this repository and prepare the workspace again before building.

Filesystem control exposes `execute_channel_until` for callers with an existing
absolute deadline. Command queuing, capability attachment and reply receipt
consume that one budget; default helpers choose 120 seconds at entry. Invalid
local command encoding returns `INVALID_ARGUMENT`. A timeout does not imply
rollback of an admitted mount operation.
