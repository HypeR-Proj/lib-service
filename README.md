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

## Parent-supervised process control

`process::control` provides one cooperative `Terminate` event with `Default`,
`Ignore`, and `Notify` dispositions. It is a userspace protocol, not a POSIX
signal ABI or a global service. The direct parent retains the Process management
handle; the protocol driver reports `Dispatch::Default` and leaves the actual
default action to that parent. It never stops a TaskGroup implicitly.

Before starting a child, call `prepare()` and move its child endpoint into the
builder under `STARTUP`, with `CHILD_RIGHTS`. Drive `Supervisor::poll` in the
parent reactor, using its dynamic `wait_item()` or `signals()` for readiness.
`request_terminate()` coalesces a pending request; it does not block or claim that
the child has exited. `Dispatch::Notified` means that the event was queued, not
that cleanup has completed. Observe the Process independently for termination.

Applications optionally claim the connection with `Client::take`, then call
`configure(disposition, absolute_deadline)` while their parent is servicing the
protocol. The initial disposition is explicitly Default, including for older
applications that never claim the connection. Registration is not implicit in
linking the runtime: applications that need a handler must register it before
relying on it. A configuration acknowledgement confirms the parent's new state.
Requests dispatched before that change retain their previous disposition.

The client receives events in ordinary thread context with `try_receive` or
`receive_until`; the library creates no worker and injects no asynchronous
callback. Drain `try_receive` before waiting, including after `configure`, which
may have saved an event while waiting for its acknowledgement. Events carry the
configuration generation under which they were dispatched. Already-dispatched
events are not retracted by later Ignore or Default configuration and must not
be mistaken for events sent to a newly installed handler.

There is at most one unconsumed notification and one coalesced further request.
A consumption acknowledgement only releases protocol capacity. Configuration
and notification writes never block the supervisor, and pending writes wait for
writable readiness instead of spinning on unread input. Malformed input closes
only that child's protocol connection; callers must isolate the error instead
of failing their entire reactor. An initial, unregistered peer close retains
Default behavior, but a disconnect after registration yields Unavailable and
never silently selects default termination. Registration timeout closes the
client connection because the caller cannot know whether its change committed.
Keep the connection alive after configuring Default or Ignore as well; dropping
a previously registered client means control is unavailable, not an implicit
request to restore Default.

Waiting for control events does not automatically cancel another channel or a
committed filesystem operation. Applications combine readiness sources and use
each operation's existing cancellation/completion contract. Cleanup, timeout
escalation, supervisor handoff and persistence guarantees belong to their owners.

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

## Userspace job membership

`process::job` supplies bounded membership records with wait-only Process
observers and request-only routes to immediate supervisors. It contains no
Process management handles and no kernel TaskGroup policy. `Registry` is driven
by its owner; capability admission can run separately using `accept`. The
registration `Client` cannot broadcast termination. `Broadcaster` is a separate,
explicitly delegated capability. `request_terminate` coalesces one pending
request locally; owners drive `poll` and include `watch` in their wait set until
the request is written. A full transport queue never counts as delivery.
Duplicating a broadcaster delegates authority without copying its pending
request, so its original owner must finish delivery before dropping it.
A registration is acknowledged after admission, and a
rejected/full admission closes or rejects its private reply rather than
retaining unbounded client state.

The shell registers a child after its builder returns a Process handle and
retains the supervisor until registration succeeds. This cooperative ordering
is not a pre-execution barrier. Registration failure terminates the directly
supervised child, but cannot roll back user code or descendants that already
ran. Native callers bypassing the protocol remain subject to existing kernel
capability/resource boundaries, not implicit userspace job membership.

A dead supervisor route is retained as unavailable until the member itself
terminates. Group fan-out is a request, not proof that each parent acted; callers
must observe actual process completion. The registry never substitutes default
termination for an unavailable route.
