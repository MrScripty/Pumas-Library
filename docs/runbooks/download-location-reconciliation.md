# Recover a failed download whose files moved

Open the library and find **Failed download recovery**. This section uses the
saved download identity, including failures that no longer join a catalog row.
Choose **Inspect existing files** for the failed download. Inspection changes nothing.

If all selected files are found, review the exact model ID, current directory,
file count and size. If several artifacts match, explicitly select the one you
want. Choose **Keep existing files and resolve failure** to retain those files
and settle the obsolete attempt. The library and download list refresh in the
same window; no restart or offline upgrade is needed for a schema-7 library.

The first confirmation upgrades the HF partition to format 6. Older applications
opening this library directly will refuse it until their Pumas dependency is
updated. The dialog warns before confirmation. Cancel leaves the format unchanged.
Applications connecting to an upgraded RPC server have a separate wire contract;
see the [compatibility evidence](../plans/download-location-reconciliation/reports/m1-evidence.md).

Local acceptance preserves the original Error and releases its exact persisted
and live download claim. It does not assert historical upstream provenance,
produce a successful HF receipt, or promise that the model can run. It does not
read/hash, copy, move, delete or back up model files. SQLite is a catalog of the
current local artifact and receives no repair transaction from this action.

Missing files or selected partials stay retained. Busy work must finish before
inspection can proceed; use **Inspect again**. A changed observation requires a
fresh inspection. If confirmation's response is lost or publication is uncertain,
use **Retry confirmation** with the same operation. Do not manually edit the
history or substitute another weight based on its display name or quantization.

This action covers legacy Error attempts with an absent original destination,
explicit selected-file evidence and no immutable revision. Modern acquisition,
intent/deletion custody and quarantined history retain their existing owners.
The separately supported pre-schema-7 metadata upgrade remains a different flow.
Downgrading a binary does not downgrade the HF partition; use a compatible reader.
