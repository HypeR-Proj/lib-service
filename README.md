<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# lib-service

Typed startup and wire contracts between Native services.

Contains console, filesystem, I/O, process, session, stdio and VM protocol modules.
Clients and services share capability-purpose identifiers and record validation while
each service retains its own state owner.

See the [implementation entry](src/lib.rs) and [component declaration](component.json)
for the public interfaces, build dependencies and delivered artifacts.

## Boundaries

These protocols sit above the kernel ABI. A type or purpose identifier does not
manufacture the corresponding capability; deployments must explicitly route the required
authority.

## Build and test

Use this checkout inside the [repo-managed workspace](https://github.com/HypeR-Proj/HypeR).
From this repository directory:

```sh
make
make test
```

An absolute `OUT=/path/to/out` selects a shared output directory. The module
entry delegates to build/system, which prepares the installed SDK and declared
dependencies. Native binaries require HypeR; host module checks cover the tests
selected by component.json. A standalone clone does not contain the workspace’s
other source repositories or Native SDK.


Filesystem control exposes `execute_channel_until` for callers with an existing
absolute deadline. Command queuing, capability attachment and reply receipt
consume that one budget; default helpers choose 120 seconds at entry. Invalid
local command encoding returns `INVALID_ARGUMENT`. A timeout does not imply
rollback of an admitted mount operation.
