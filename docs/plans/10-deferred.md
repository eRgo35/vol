# M10 — Backlog (deferred, post-0.1)

**Status:** Not started · **Size:** — · **Depends on:** M9 · **Branch:** `(none)` · Index: [`PLAN.md`](../../PLAN.md) · Roadmap: [`ROADMAP.md`](../../ROADMAP.md#m10--backlog-deferred-post-01)

**Goal.** Specified-as-deferred features; not scheduled. Each becomes its own milestone when promoted.

## Requirement IDs assigned to this milestone (4)

`VOL-PB-010`, `VOL-OUT-012`, `VOL-IN-008`, `VOL-CFG-009`

Specified-as-deferred features and unscheduled ideas. **Not part of v0.1.0.** A feature is promoted by: (1) adding/adjusting its spec section, (2) creating a milestone entry in `ROADMAP.md`, (3) writing a plan file in this directory. Verification stays local (`just check`) for all of them.

## Specified deferrals (assigned to this milestone for coverage)

| ID | Feature | Preconditions before promotion | Sketch |
|---|---|---|---|
| VOL-PB-010 | Terminate stream (`x`) on Playback/Recording | Verify `Registry::destroy_global` on a foreign stream node (permissions; WirePlumber re-creates? app reaction); decide confirm prompt | key `x` → confirm popup → `Command::Terminate{node}`; status `Stream terminated`; disappearance handled by existing flows |
| VOL-OUT-012 | Latency offset ("Advanced") | VF-13: does WirePlumber/ACP honor `Route.props.latencyOffsetNsec` vs node `ProcessLatency`; unit/range | `a` opens a number popup (ms, step 1/10); write via the verified param; show on row line 3 when ≠ 0 |
| VOL-IN-008 | "Monitor of …" entries in Input Devices and moving capture streams to monitors | VF-10 outcome from M4 (does metadata moving to a sink serial work without `stream.capture.sink`) | synthetic rows (no node, no volume), `SinkMonitor` meter; move popup adds monitor entries only if VF-10 positive |
| VOL-CFG-009 | Configuration "Show: All Devices" filter; Bluetooth codec selection | Define what the filter hides (e.g. cards without nodes); inspect `bluez` device params for codec properties | `DeviceFilter`-like enum on the tab; codec popup if device `Props` expose it |

## Unscheduled ideas

See ROADMAP §Backlog. Each needs a spec first. Notable dependencies: persistence features (UI state, key remap) require a config/state file design (Q12); ASCII/alternate glyph sets require Q9 to be reopened; distro packages and aarch64 artifacts require extending `release.yml` (the only workflow) and a build matrix, still without any verification job.
