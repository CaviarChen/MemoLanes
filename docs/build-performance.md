# Local build performance

Measured on 2026-09-12, starting at commit `3ed97475`. Keep using `flutter run`:
the retained optimizations require no wrapper, shell alias, or build-mode toggle.

## Retained changes

- Enable `org.gradle.parallel=true` and `org.gradle.caching=true` in Android's
  `gradle.properties`. These change task scheduling and output reuse, not compiler
  optimization, ABI selection, shrinking, or signing settings.
- Put ordinary Cargo output in the repository's `.build-cache/rust`, outside
  Flutter's `app/` tree. Move the Cargo configuration from `app/rust/.cargo/` to
  the repository root so invocations from `app/` and the root using
  `--manifest-path` also respect it. Native Assets supplies its own explicit
  target directory and keeps its existing cache.
- Remove the optional Rust dev-mode toggle and fixed `dev` Git label. Native
  Assets always uses Rust release mode and embeds the actual Git revision.
  Keep the existing generated-file writes that only update changed content.
- Preserve CI's per-directory Cargo caches with `CARGO_TARGET_DIR=target` in the
  App workflow. Also cache `app/target`, used by its `--manifest-path` commands.
  Root Cargo configuration changes now trigger that workflow.

Cargo searches configuration from the invocation directory upward, rather than
starting at the `--manifest-path` crate. This explains why the earlier nested
configuration allowed `app/rust/target` to reappear. Environment variables and
explicit target-directory arguments can override the checked-in setting.
See [Cargo configuration](https://doc.rust-lang.org/cargo/reference/config.html).

Existing caches need a one-time move while no Rust build is running; changing
configuration does not remove old directories. On the benchmark machine the
5.1 GB `app/rust/target` was moved into `.build-cache/rust`; previous caches were
preserved separately under `.build-cache/`. Do not overwrite an existing cache
directory. No cache migration runs automatically during builds.

## Measurements

Environment: macOS / Apple Silicon, Flutter 3.47.2, Dart 3.13.2, Rust 1.98.0,
Gradle 9.3.1, AGP 9.1.0, Kotlin 2.4.0, Android Studio JBR 21.0.10. Gradle used
the existing 4 GB heap and 10 worker leases. The Android build targeted ARM64.

| Scenario | Before (seconds) | After (seconds) | Interpretation |
| --- | --- | --- | --- |
| Android: force the same 655 tasks, serial vs parallel | 30.038, 20.802 | 19.021, 18.149 | Later pair is about 13% faster; first serial run includes warm-up effects |
| Android: rebuild 21 plugins whose output directories were temporarily moved aside, cache off vs on | 6.096, 4.817 | 4.154, 3.103 | Median 5.46 → 3.63 s, about 34%; 243 tasks restored from cache |
| Android: unchanged warm build, serial/cache off vs parallel/cache on | 17.363, 7.828, 5.962 | 5.058, 4.024, 8.143 | High variance and warm-up effects; no reliable no-change speedup established |
| iOS prerequisite: recursive xattr cleanup of a copied Rust tree, cache inside vs outside | 3.538, 3.783, 4.051 | 0.027, 0.026, 0.027 | Median 3.783 → 0.027 s for this component only |

Android values measure the Gradle subprocess with the project properties emitted
by `flutter run -v`; they exclude device installation, VM connection, and the
rest of the Flutter launcher. They are not end-to-end `flutter run` timings.
The forced-task comparison alternated serial / parallel / serial / parallel,
with build cache enabled on both sides and `--rerun-tasks` forcing execution.
The plugin-output comparison alternated cache off / on / off / on, with parallel
execution enabled on both sides. Original output directories were restored after
each sample. No `flutter clean` was used.

The iOS measurement used an independent APFS copy of the Rust tree in a temporary
directory. It ran `xattr -r -d` for `com.apple.FinderInfo` and
`com.apple.provenance`, first with Cargo targets inside, then with them outside
that copy. It did not modify the original source tree's metadata. Flutter 3.47.2
performs these recursive operations in `flutter_tools/lib/src/ios/mac.dart`;
that code does not consult `.gitignore` or Dart analyzer exclusions. This is a
component benchmark, not a new end-to-end iOS run or IPA verification.

### Dependency-service limitation

The initial ordinary Android run could not resolve the MMKV version range:
`mmkv_android` 2.4.2 requests `com.tencent:mmkv:[2.4.2, 2.5)`, and Maven Central's
metadata endpoint returned HTTP 404 during this session. Offline resolution
also lacked the cached version listing.

Controlled comparisons used a temporary, app-scoped Gradle init script providing
a local Maven repository with the already cached MMKV 2.4.2 AAR/POM/module and
a version listing. Dependencies were warmed first and timed comparisons then
used `--offline`. Neither dependency declarations nor repository configuration
were changed in the project. The measurements therefore isolate build execution
and do not establish performance or reliability of online dependency resolution.

Final `flutter run --no-pub --no-resident` validation on the connected Android
PGT110 completed successfully: APK built, installed, launched, and connected to
the Flutter view. It used a temporary Gradle user home with the same test-only
MMKV repository and offline mode. This first run after the hook changes took
66.764 seconds; there is no equivalent successful end-to-end before sample, so
it is a functional check rather than a speedup measurement.

## Release verification

Built an Android ARM64 Release baseline before source changes, then rebuilt with
the final changes. Also forced all 708 Release tasks to execute, and repeated
with both Gradle optimizations disabled as a control. Existing R8, resource
shrinking, icon tree shaking, Rust release compilation, and the project's
existing signing configuration were retained.

- All 748 uncompressed APK entries have the same SHA-256 as the baseline,
  including DEX, native libraries, manifest, assets, and resources.
- APK size remains 45,807,290 bytes. The initial incremental after-build also
  matched the baseline's entire APK hash; forced rebuilds do not.
- Comparing the full serial and parallel rebuilds isolates every differing byte
  to block `0x504b4453` in the APK signing container (dependency metadata).
  All other bytes, including ZIP metadata and the v2 signature block, match.
  Android documents that dependency metadata is encrypted and stored in this
  container; see [dependency information](https://developer.android.com/build/dependencies).
- `apksigner verify --verbose` passes with APK Signature Scheme v2.

The evidence supports unchanged Android Release application content, not a
promise of reproducible whole-APK hashes or validation of every platform/ABI.
No iOS Release IPA or distribution-signing comparison was performed. Release
timings are not used as speedup evidence because the task/cache states differed.

## Options not retained

- **Gradle configuration cache:** failed with six compatibility problems in the
  current Flutter Gradle tasks, including `:app:DebugMinSdkCheck` capturing
  unsupported Gradle/variant state. The cache entry was discarded. Do not enable
  it or suppress its errors until the tooling supports it.
- **Rust dev profile controlled through pubspec or an environment switch:** the
  Native Assets hook does not inherit Flutter's build mode. A manual toggle adds
  a release-mode mistake risk and changes native optimization. Removed.
- **Flutter SDK patches, command wrappers, or custom scan-exclusion machinery:**
  unnecessary maintenance for a problem solved by a Cargo output location.
- **Larger JVM heaps, extra worker tuning, dependency pins, or skipping build
  checks:** no measured evidence here to justify them.

For Dart-only iteration, hot reload/restart in a resident `flutter run` session
still avoids the launch/build cycle. Run `just pre-build` when its generated
inputs change; it is not required before every launch.

## Reproducing the comparison

Capture the Gradle command and `-P` properties from `flutter run -v`. Run that
same command sequentially from `app/android`, with dependencies available:

1. Warm it once. Keep Java, device architecture, sources, and dependency versions
   fixed throughout the comparison.
2. Compare `--build-cache --rerun-tasks --no-parallel` with
   `--build-cache --rerun-tasks --parallel`, alternating and repeating samples.
3. Compare `--parallel --no-build-cache` with `--parallel --build-cache` while
   temporarily moving the same plugin output directories aside for each sample.
   Restore the originals after each run, including failed runs.
4. Use `--no-configuration-cache` for both. Record process wall time and the
   executed / up-to-date / from-cache task counts. Keep network time separate.
5. Build Release on both sides; compare each ZIP entry's content hash and verify
   signatures, rather than inferring equality from build success or APK size.

The local raw logs, runner scripts, APK entry hashes, and comparison JSON are in
`/private/tmp/memolanes-build-bench/` on the measurement machine. They are
temporary diagnostics, not build dependencies. Gradle's mechanisms are described
in [build cache](https://docs.gradle.org/current/userguide/build_cache.html) and
[build performance](https://docs.gradle.org/current/userguide/performance.html).
