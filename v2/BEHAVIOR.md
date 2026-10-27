# Behavior and explicit boundaries

## Source contract

Own source is `almatkai/node-release-notes` at
`2410143f530d3d6ea7143e25717322acc4af7512` (fresh `origin/main` did not advance).
Upstream manifest/assets are from Raycast commit
`6734d85992290066fd31a7f5bee3cca3d3545a1a`, original author **fil**, no manifest
contributors field. Own Git src history credits Almat. Exact Git-object hashes
are in `qa/git-objects.json`; immutable downloads are in `qa/immutable-captures.json`.

The source validator requires a nonempty array, records with string `version`
and `date`, and absent/false/string `lts`. `true` and `null` are invalid. Fields
such as `files`, `npm`, `security`, `isCurrent`, etc. add no validation requirements.
The port retains those decisions and the actual source error messages, within
explicit size bounds. A nested array record fails its missing `version` check,
as the real JavaScript validator does.

Current is only feed index zero; Latest compares the previous **adjacent**
record's `version.split('.')[0]`; LTS is a truthy nonempty LTS string. Tag order is
Current, LTS, Latest. `isCurrent` is ignored. There is no semver sorting. Groups
preserve first-seen feed major order and intra-group order, including duplicates
and nonconsecutive majors. Default filter is `latest`, one first row **per group**;
LTS filters truthy LTS strings; All's value is the empty string.

Search matches titles and the source's tag-label keywords, split by `': '`, with
case-insensitive substring matching. This is a documented finite native matching
rule, not a claim of equivalence with Raycast's unspecified fuzzy matcher. The
host's existing debounced search callback sends text to the process; filtering is
local to the already fetched feed. No keystroke performs HTTP. Search, filter and
list data survive detail/Back in the session. No dropdown value is migrated or
persisted across launches: the actual own host ignores `storeValue`. The patch
adds controlled `value` support so Back restores the selected filter accurately;
it does not invent persistence.

List rows use the exact `nodejs-icon.svg`, version, source tag order and local
date accessory/tooltip. Row action order is Show Release Notes, Copy Version.
The dropdown callback is registered separately, not exposed as an incomplete
menu action. Duplicate rows have unique index IDs even when versions repeat.

Detail requests `/repos/nodejs/node/releases/tags/<version>`. Its raw template is
exactly the source `# version`, `## Release Name:`, nullish name fallback, then
truthy body / error message / empty fallback. Tests compare the genuine source
template before native safe rendering. Published, release URL, exact
`https://nodejs.org/docs/<version>/api/`, and **captured list tags** are retained.
The payload preserves source tag color/calendar-icon tokens; the baseline host
uses its own limited tag palette (Current/Detail colors can be neutral) and does
not draw the calendar accessory icon. This is a renderer styling adaptation, not
a claim of pixel parity.
Detail action order: Open Release Notes, Open Docs, Copy Version, Copy Release
Notes URL, Copy Docs URL. Back is additional navigation. Failed/absent/malformed
responses are visible errors with deliberate Retry; they are never successful
empty results. Retry is an explicit addition equivalent to the locked utility's
failure-toast Retry path.

## Fetch/cache/lifecycle

The actual locked `@raycast/utils 1.12.5` chain is executed in the oracle, including
`useCachedPromise`, `usePromise`, and `useCachedState`. Every mount revalidates;
cache survives mounts, previous successful data can remain alongside an error,
and a failure toast offers deliberate Retry. Neither source hook polls or has
a silent TTL.

Native uses private `dataDir/node-cache.json`, at most **8 MiB**, one successful
feed plus at most **16 whole release entries**, evicting oldest whole entries
when necessary. This is a bounded native cache adaptation; it does not read or
migrate Raycast caches or preferences. Every list launch/detail entry still
revalidates. Successful data is cached atomically. Failed revalidation retains
last successful data plus an explicit error/Retry. Invalid/unwritable cache is
reported; cache limits never truncate displayed records or release bodies.
There is no TTL, polling, auth or key. All views return `refreshMs: null`.
Native open/detail operations wait for the bounded fetch and return a settled
view. They rely on the host command busy surface during that wait, rather than
streaming Raycast's intermediate `Loading...` node; real GUI acceptance is not run.
Refresh/search/load-more do not fetch or repeat OS effects.

The SDK stages Value state and commits only successful current operations.
Search ownership is its reader-order latest-query contract. No adapter detached
tasks or unowned `tokio::spawn` exist. A single event-driven SDK-owned worker carries
a Drop guard for each authoritative registration, bounded to 128 sessions and
released on canceled/failed open, dispose, EOF or shutdown, including opens whose
host never sends dispose. Guards are constructed before scheduling and compare
Arc identity before removal.

Each displayed view has a monotonic action epoch and bounded action table
(maximum two row actions per 12,000 records plus navigation/retry callbacks).
An action consumes its displayed epoch **before** awaiting HTTP or a host effect.
Old/discarded IDs fail. Cancellation discards staged navigation/details. Late
network responses cannot commit. Rendering a discarded view may conservatively
revoke its predecessor; a read-only refresh issues fresh IDs and performs no
network/OS operation. A dropped action guard signals a coalesced Notify; the
session-owned worker emits one invalidation after a 10 ms cleanup yield so the
actual host can recover the current view/uncertain retry after cancellation. It
waits on events and does not poll or replay effects.

OS effects cannot be rolled back with Value state. One authoritative prepared
Copy/Open checkpoint therefore retains exact bytes/URL before the reverse RPC.
Canceled/failed/malformed acknowledgements leave an uncertain outcome. Explicit
Retry repeats exactly those prepared bytes/URL without refetching; Dismiss clears
the checkpoint. There is no automatic replay or ordinary delivery confirmation.
List row action menus also expose Retry/Dismiss during uncertainty. Confirmed
success clears the checkpoint; old IDs remain consumed.

Only typed `host.clipboard.write {text}` and `host.shell.open {target}` are used.
Grants are exactly `clipboard.write`, `shell.open`. The actual host clipboard
shape is `{ok:true}`; open additionally permits a **string** `message`. Null,
booleans, non-true ok, unexpected keys, non-string message and any simultaneous
`error` field (even null with ok:true) are rejected. Generic renderer code-copy
controls remain separate from primary Copy Version and validated metadata links.

## HTTP and untrusted content hardening

Native reqwest/rustls TLS verifies certificates, has no proxy/config/env endpoint
override, and permits only fixed HTTPS origins and exact routes:

- `nodejs.org/dist/index.json`
- `api.github.com/repos/nodejs/node/releases/tags/<safe-version>`

No redirects are followed (**zero** allowed). Connect deadline is 4 seconds;
whole request/body deadline is 12 seconds. User-Agent is
`Tezbar-Node-Release-Notes/2.0.0`; Accept is
`application/vnd.github+json, application/json`; GitHub API version is 2022-11-28.
Bodies are at most 2 MiB, including unknown-length/chunked bodies; bad status,
truncation, invalid JSON and overflow are errors. No silent output truncation.
JSON bounds: depth 32, 200,000 nodes, each string 1 MiB, each key 4096 bytes.
Feed: 12,000 records; version/LTS 256 bytes, date 128. Release name: 4096 bytes;
body: 1 MiB; published_at: 128; html_url: 2048. Rendered view: 12 MiB, below the
SDK's 16 MiB frame cap. Queries are at most 1024 UTF-8 bytes. Node version
navigation is capped at 128 ASCII bytes, `v<digits>.<digits>.<digits>` optionally
followed by `-` and alphanumeric/dot/dash suffix. Slash, backslash, percent,
query/fragment/control characters cannot become a new path/origin. Docs use the
same validated segment. Remote open URLs must be canonical HTTPS GitHub
`/nodejs/node/releases/tag/<safe-version>` with no credentials/port/query/fragment.

These limits and URL/field checks deliberately harden the loose source runtime
validator and TypeScript-only detail fields. Malformed/prototype-name versions
are displayable but navigation fails visibly; native grouping does not reproduce
JavaScript prototype-object exceptions. Oracle records those exceptions rather
than claiming universal compatibility. Arbitrary detail objects/primitives,
missing release URLs/dates, non-JSON detail bodies and unsafe URLs are errors.
Nullable/missing name/body retain the source empty-string fallback.

The exact source Markdown template is parsed with pulldown-cmark into safe HTML.
Raw HTML becomes text. All body links/images become inert label and URL text;
there are no remote image loads, tracking requests, relative/local file image
resolution, arbitrary host links or forged UI/code-copy buttons. Markdown headings,
code, tables, emphasis and strikethrough remain readable. The actual pinned
renderer then performs its normal Marked/DOMPurify pass. This is an honest native
UI adaptation, not pixel/Markdown-engine equivalence. Metadata/action URLs are
independently validated and remain available. Renderer tests use malicious API
fixtures and the actual code-copy effect with capture-only clipboard services.

## Date/locale adaptation

The source uses `new Date(...).toLocaleDateString(undefined,{dateStyle:'short' /
'long'})` and `.toLocaleString()` in the user's JS environment. The finite oracle
fixes clock to 2025-01-02T00:00:00Z, locale **en-US**, timezones **UTC** and
**America/Los_Angeles**. The native result matches those fixture cases, including
UTC date-only parsing, local rollover, leap date, invalid/empty dates and published
seconds. Native uses the system local timezone, a fixed English Gregorian
short/long/datetime format and ISO date-only or RFC3339 parsing. Other locale/date
syntaxes are an explicit adaptation; unsupported parses display `Invalid Date`.
There is no claim of universal Date/Intl or regional OS-locale equivalence.
