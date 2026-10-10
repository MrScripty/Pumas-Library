# Opening an older library

If the desktop app shows **Library upgrade required**, the selected library's
download database needs an explicit offline upgrade. Opening or selecting a
library never performs this upgrade automatically. The app stays open so you
can select another library or close it.

If it shows **Library could not start**, first close other apps using that
library and reopen Pumas Library. The app's main log retains the startup failure;
an unavailable backend never becomes an empty or ready library.

## Upgrade on Linux

1. Close Pumas Library and every other reader/writer using this library,
   including older backend processes, developer instances and library consumers.
   They must remain stopped throughout the upgrade. Current-owner exclusion
   cannot prove that historical processes have stopped.
2. Decide to move this library to the new application version. Schema 7 does
   **not** support automatic downgrade or old-binary rollback. Do not reopen
   this upgraded library in older apps. Back up the complete library separately
   if you need to retain a separate old-version installation.
3. Run the installed backend's explicit upgrade command, replacing the quoted
   example root with the library root you selected:

   ```bash
   "/opt/Pumas Library/resources/pumas-rpc" \
     --migrate-download-store-offline \
     --launcher-root "/path/to/your/library" \
     --confirm-old-writers-stopped
   ```

   This command starts no server. It refuses a cooperating current owner,
   saves an exact private backup as
   `launcher-data/downloads.pre-schema7-<id>.json`, syncs that backup before
   migration, then invokes the canonical v4/v5/schema-6 converter. It preserves
   download and acquisition facts without fabricating historical completion
   receipts. It does not rewrite your model files.
4. Check that the command reports an upgrade to schema 7 and identifies the
   backup. Reopen the new Pumas Library app with the same selected root.

An older failed startup may already have left a `claiming` entry in the instance
registry. The new schema preflight prevents creating that entry on a refused
legacy store, but the database upgrade does not retire an existing claim. If the
upgraded library still reports an owner after every app has closed, preserve the
main log and registry for ownership recovery; do not delete the registry or infer
safe reclamation from a dead PID alone.

On failure, keep the original database and any backup. Do not delete
`downloads.json`, repeatedly reset the library, or automatically restore an
old-schema backup: a publication failure can leave the new schema visible.
Resolve the reported failure before retrying. Already-current stores,
unsupported/corrupt documents and platforms without a qualified physical store
lease are refused without an automatic conversion.

The migration and historical-writer obligations are owned by the
[artifact acquisition contract](contracts/artifact-acquisition.md#9-persistence-and-consumer-finalization).

## Startup diagnostic boundary

The RPC startup supervisor emits a newline-delimited, path-free stdout frame
`PUMAS_STARTUP_FAILURE={"version":1,"reason":"migration-required"}` only
for the typed `acquisition.migration_required` failure, before exiting nonzero.
The Electron bridge reads bounded complete lines across chunks and waits for
stdio closure before consuming a failed-start diagnostic. Unknown versions,
malformed frames and other startup failures become `backend-unavailable`.

Ordinary HF-enabled owners check an existing download store while holding the
physical library lease, before claiming the registry or starting background work.
An unsupported schema therefore leaves no new instance claim behind: reopening
continues to show the upgrade message. Constructors using a reserved authority,
and failures after startup effects begin, retain their existing custody rules.

The desktop root state remains `initializing` until backend readiness. Failed
starts publish `recovery-required` through main, the validated preload and the
renderer; they never retry as runtime crashes. Recovery allows library selection
for saved/default roots and preserves environment/argument authority for explicit
roots. The release smoke path still fails for either recovery state. Post-readiness
runtime restart and shutdown-failure receipts retain their existing meaning.
