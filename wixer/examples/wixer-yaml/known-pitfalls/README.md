# Common WiX Pitfalls Gallery

This directory contains examples of common mistakes in WiX installers. Each example demonstrates a specific pitfall with documentation on how to detect and fix it.

## Expected Build Failures

These examples are intentionally invalid and are not meant to build into an MSI package.

When running `cargo make gallery-build`, the `known-pitfalls` entry is configured with `expect_build_failure: true`.
This means:

- A failing build is treated as success and shown as `✓ (expected failure)`.
- A successful build is treated as an error (because the pitfall is no longer causing a build-time failure).

## Pitfalls

| # | Pitfall | Detection Method |
|---|---------|------------------|
| 01 | [Typo in Condition](#01-typo-in-condition) | `wixer simulate --verbose` |
| 02 | [Action After InstallFinalize](#02-action-after-installfinalize) | `wixer simulate` |
| 03 | [Orphan Component](#03-orphan-component) | LSP diagnostics |
| 04 | [Impossible Condition](#04-impossible-condition) | `wixer simulate --verbose` |
| 05 | [Wrong Install Mode](#05-wrong-install-mode) | `wixer simulate` with multiple modes |
| 06 | [Publish Order Conflict](#06-publish-order-conflict) | LSP diagnostics |
| 07 | [Missing Override](#07-missing-override) | LSP suggestions |
| 08 | [Override Wrong Target](#08-override-wrong-target) | `wixer describe` comparison |
| 09 | [Action Override Typo](#09-action-override-typo) | `wixer simulate` |

## 01: Typo in Condition

**File:** `01_typo_in_condition.wxs.yaml`

**Problem:** Misspelled condition property name (e.g., `FistInstall` instead of `FirstInstall`)

**Symptom:** Custom action is always skipped because undefined properties evaluate to false.

**Detection:**

```bash
wixer simulate 01_typo_in_condition.wxs.yaml --mode first-install --verbose
# Look for: CA_FirstInstallOnly shows "○ skip" with condition "FistInstall"
```

**Fix:** Correct the typo in the condition.

---

## 02: Action After InstallFinalize

**File:** `02_action_after_finalize.wxs.yaml`

**Problem:** Deferred custom action scheduled after `InstallFinalize`

**Symptom:** Action runs outside the installation transaction, meaning failures won't trigger rollback.

**Detection:**

```bash
wixer simulate 02_action_after_finalize.wxs.yaml --mode first-install
# Look for: CA_TooLate at sequence > 6600 (after InstallFinalize)
```

**Fix:** Change `After: InstallFinalize` to `Before: InstallFinalize`.

---

## 03: Orphan Component

**File:** `03_orphan_component.wxs.yaml`

**Problem:** Component defined but not referenced by any Feature.

**Symptom:** Component's files are compiled into the MSI but never installed.

**Detection:** Open the file in VS Code with wixer-vscode extension. The LSP will show:

```text
⚠️ Component 'OrphanComponent' is not referenced by any Feature
```

**Fix:** Add a `ComponentRef` to a Feature, or include the component in a `ComponentGroup` that is referenced.

---

## 04: Impossible Condition

**File:** `04_impossible_condition.wxs.yaml`

**Problem:** Condition that is logically impossible (e.g., `Installed AND NOT Installed`)

**Symptom:** Action never runs in any installation scenario.

**Detection:**

```bash
# Test all modes - action should be skipped in ALL of them
wixer simulate 04_impossible_condition.wxs.yaml --mode first-install --verbose
wixer simulate 04_impossible_condition.wxs.yaml --mode upgrade --verbose
wixer simulate 04_impossible_condition.wxs.yaml --mode uninstall --verbose
```

**Fix:** Review and correct the condition logic.

---

## 05: Wrong Install Mode

**File:** `05_wrong_install_mode.wxs.yaml`

**Problem:** Action intended for uninstall uses `FirstInstall` condition.

**Symptom:** Cleanup action runs during install (wrong) but not during uninstall (when needed).

**Detection:**

```bash
# Compare behavior across modes
wixer simulate 05_wrong_install_mode.wxs.yaml --mode first-install --verbose
# Shows: CA_CleanupOnUninstall RUNS (unexpected)

wixer simulate 05_wrong_install_mode.wxs.yaml --mode uninstall --verbose
# Shows: CA_CleanupOnUninstall SKIPPED (should run!)
```

**Fix:** Change condition from `FirstInstall` to `REMOVE~="ALL"` or use `Uninstalling` property from `InstallationStages.wxi.yaml`.

---

## 06: Publish Order Conflict

**File:** `06_publish_order_conflict.wxs.yaml`

**Problem:** User-defined Publish element has same Dialog/Control/Order as UI extension.

**Symptom:** Both events fire at the same order, causing unpredictable behavior.

**Detection:** LSP shows "Publish Order conflict with UI extension" warning.

**Fix:** Use a different Order value, or use `override` to replace the extension's event.

---

## 07: Missing Override

**File:** `07_missing_override.wxs.yaml`

**Problem:** User wants to replace UI extension's navigation but doesn't use override.

**Symptom:** Both user's event AND extension's event fire, causing double navigation.

**Detection:** LSP suggests using "override" for same-type events.

**Fix:** Use `Event: override <target>` syntax to replace the extension event:

```yaml
UI:
  Publish:
    Dialog: InstallDirDlg
    Control: Next
    Event: override VerifyReadyDlg
    Value: MyCustomDlg
```

---

## 08: Override Wrong Target

**File:** `08_override_wrong_target.wxs.yaml`

**Problem:** Override specifies wrong target dialog name.

**Symptom:** Override has no effect - extension event still fires.

**Detection:** Compare with `wixer describe` output for UI extension events.

**Fix:** Use exact dialog name from extension. Check with:

```bash
wixer describe InstallDirDlg
```

---

## 09: Action Override Typo

**File:** `09_action_override_typo.wxs.yaml`

**Problem:** Trying to override standard action with wrong capitalization.

**Symptom:** Creates NEW action instead of overriding - both actions run.

**Detection:** `wixer simulate` shows both actions in sequence.

**Fix:** Use exact action name with correct PascalCase (e.g., `RemoveExistingProducts` not `Removeexistingproducts`).

---

## Running All Pitfall Tests

```bash
cd wixer/examples/wixer-yaml/known-pitfalls

# Quick test all examples
for f in *.wxs.yaml; do
  echo "=== $f ==="
  wixer simulate "$f" --mode first-install 2>&1 | tail -1
done
```

## Contributing

When adding a new pitfall example:

1. Use naming convention: `NN_short_description.wxs.yaml`
2. Include a detailed comment header explaining:
   - The problem
   - The symptom
   - How to detect it
   - How to fix it
3. Update this README with the new pitfall
