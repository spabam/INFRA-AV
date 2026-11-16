# Project requirements

Every source or documentation change must update the affected component and
product headline with `bash scripts/version.sh bump [COMPONENT...]`, following
`VERSIONING.md`. Builds and reinstallations validate/reuse recorded versions;
they do not bump releases. Keep generated version metadata synchronized.
