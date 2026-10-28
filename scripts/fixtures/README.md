# KDE PAM snapshots

`kde/` contains unmodified PAM configuration samples from the distributions covered
by `scripts/test-kde.mjs`. CI tests these checked-in files so an outage at Launchpad
or another distribution server does not fail unrelated commits. All distribution
and authentication scenarios still run.

`kde/manifest.json` records when the samples were fetched, their source URLs and
SHA-256 hashes. RPM entries also record the path inside the source package;
`missing: true` records an optional file that returned HTTP 404. The samples retain
their respective upstream licensing and any copyright notices in the files.

Run the same tests as CI without downloading sources:

```sh
node scripts/test-kde.mjs scripts/fixtures/kde
```

To check current distribution sources, run `node scripts/test-kde.mjs` without a
directory, or enable `test_live_kde` when manually running the CI workflow.
Download a new snapshot into an empty directory and run its tests with:

```sh
node scripts/test-kde.mjs --download /tmp/gaze-kde-next
```

After the tests pass, review the source changes and replace the whole `kde/`
directory with that snapshot, including its manifest. Replacing the directory
ensures that files removed by a distribution do not survive from the old snapshot.
