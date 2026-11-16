# INFRA-AV versioning

Every source and documentation change must update its affected component(s)
and the product headline. This follows VAPT's `10.<release>.<YYYYMMDDHHMM>` rule.

- Major is fixed at **10**.
- The release counter increases once on each day that component changes.
- The timestamp records its latest change to minute precision, using the author's
  local clock, as in VAPT. A later same-day change keeps the counter.
- Unchanged components keep their versions. The product headline always moves
  when `bump` runs. These new registries start at release 1 on adoption;
  existing source history and VAPT's historical counters are retained.

`VERSION` is the source of truth. `versioning.json` declares components and
outputs. Components for this product are `infra-av`, `backend`, `web`, `installer`; the headline is
`infra-av`. Use the named component matching the files changed;
installer/build/version tooling belongs to `installer`.

```bash
bash scripts/version.sh show
bash scripts/version.sh value
bash scripts/version.sh bump backend
bash scripts/version.sh bump                 # product-wide documentation only
bash scripts/version.sh sync                 # regenerate metadata; no bump
bash scripts/version.sh check                # read-only consistency check
```

`bump` validates all component names and timestamps before writing, automatically
updates the headline, and regenerates `version.json` plus the declared API, CLI
and interface artifacts. Do not edit generated files directly. `--at YYYYMMDDHHMM`
is available for explicit release preparation; timestamps cannot move backwards.
Two changes in the same minute share the timestamp, matching VAPT's precision.

Builds and installers validate/reuse the recorded source release; rerunning an
installation never invents a release or changes its timestamp. UUUUI's installer
validates selected versioned modules, rebuilds them and publishes their metadata.
Standalone UI builds also validate versions. Package-manager versions, database
migration numbers, protocol versions and external dependencies are independent
and are not rewritten to these product release numbers.

The portable source bundle includes this policy, the registry, configuration,
self-contained standard-library Python helper and generated metadata. It excludes
`.version.lock`, caches, credentials and runtime state. Each product carries its
own helper so it can be copied or checked independently of its original parent
checkout; the canonical implementation is `UUUUI/scripts/version.py`. Keep its
copies synchronized when maintaining the helper. Source files remain owned by
`spabam:spabam` on machines where that account exists.
