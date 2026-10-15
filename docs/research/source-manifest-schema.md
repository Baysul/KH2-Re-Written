# Source manifest schema v1

Owner: `source`. Stored privately outside source/Git. JSON fields:

| Field | Type / meaning |
|---|---|
| schema_version | Integer 1; future changes must migrate explicitly |
| canonical_source_path | OS canonical source path, private and machine-specific |
| product | Candidate KH2FM product description, not a supported-build claim |
| observed_app_id / observed_build_id / observed_steam_language | Optional Steam scalar clues, bound only when app/installdir match selected root |
| qualification | Currently always `quarantined_unqualified`; no approved profile exists |
| edition_confidence | Explicit evidence limit; edition is not inferred as verified |
| content_identity | SHA-256 of schema-domain marker and sorted relative path/size/SHA-256 tuples; unaffected by relocation/timestamps |
| files | Ordered observations with `relative_path`, `size` (u64), `modified_unix_nanos` (u128), SHA-256 hex, and candidate `role` |
| unknowns | Required unresolved qualification/language/dependency facts |

Paths in portable records use forward slashes. The manifest hashes the candidate files listed in `supported-builds.md`; it does not enumerate unrelated collection games. Unknown shared dependency closure is a blocker, not an empty-set pass. `content_identity` is content evidence and is never an account-ownership, license, authenticity, or compatibility proof.

Reproduction: use the optimized scanner on the same explicitly selected idle source twice with two new private output filenames, compare `content_identity` plus file hash/size/mtime records. Preserve both originals and scanner/rewrite commit identity. No partial successful manifest is written if any candidate read/hash fails.
