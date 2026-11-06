# Compatibility rules

Rules have stable IDs in report schema version 1. Each finding includes contract, consumer when applicable, previous/proposed signature and topic0 when available, category, certainty, field, values, explanation, and remediation.

| ID     | Severity          | Detection and example                                                                                                                                                                                                                                                |
| ------ | ----------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| EAG001 | breaking          | Required exact baseline signature absent from proposed ABI. Adding a parameter removes `Payment(address,uint256)` even if a same-name event exists.                                                                                                                  |
| EAG002 | warning, possible | One removed and one added unmatched same-name event suggest a changed signature. EAG001 remains the definite consumer failure; this rule never asserts replacement identity.                                                                                         |
| EAG003 | breaking          | A decoded consumer's event changes any input's indexed flag, including inputs not listed in fields. Signature/topic0 may remain equal.                                                                                                                               |
| EAG004 | breaking          | A required field no longer resolves and there is insufficient same-position/type evidence to describe a rename.                                                                                                                                                      |
| EAG005 | breaking          | A resolving required field changes canonical type. A changed exact type sequence is already caught by EAG001; no uncertain replacement is used for field comparison.                                                                                                 |
| EAG006 | breaking          | A required name no longer resolves but an equal-type slot remains; an explicit name at a position changed; or nested component labels of a required named tuple changed. It proves failure to satisfy the name expectation, not semantic identity of renamed fields. |
| EAG007 | breaking          | A uniquely named required field is now at a different position within the same canonical event signature.                                                                                                                                                            |
| EAG008 | breaking          | Anonymous status changes for a decoded dependency, or a topic-only event becomes anonymous. Signature hash equality does not preserve emitted topic layout.                                                                                                          |
| EAG100 | warning / info    | Structural diff without a declared consumer implication. Additions are info; all other changes are warnings. A possible same-name relationship and its explanatory parameter changes have possible certainty.                                                        |

EAG004/EAG005 are field-level classifications when exact-signature comparison supplies enough evidence. Most field count/type changes necessarily remove the required signature and therefore report EAG001 instead of speculative downstream field violations. Redundant cascading errors are deliberately avoided.

## Structural categories

`EVENT_ADDED`, `EVENT_REMOVED`, `EVENT_SIGNATURE_CHANGED`, `EVENT_PARAMETER_TYPE_CHANGED`, `EVENT_PARAMETER_ADDED`, `EVENT_PARAMETER_REMOVED`, `EVENT_INDEXED_CHANGED`, `EVENT_PARAMETER_RENAMED`, `EVENT_PARAMETER_REORDERED`, and `EVENT_ANONYMOUS_CHANGED`.

Exact signature matches provide definite structural properties. Name/type sets that are unchanged and uniquely named permit a description of the named parameter order changing; this is an ABI-label ordering observation, not a guarantee of preserved business identity. Tuple component changes use positional locations such as `0.1.2`. When identities cannot be established across differing signatures, detailed changes are only possible. Ambiguous same-name overloads produce definite added/removed events with no inferred pairing. Renamed event names are removals/additions without a guessed cross-name relationship.

## Severity versus certainty

Severity describes policy impact; certainty describes the evidence. A definite unused removal is a warning, while a definite required removal is breaking. A same-name candidate is always possible and never breaking by itself. Structural findings remain visible alongside consumer violations; severity counts count findings, not unique events or consumers.

Changing indexed flags alone never causes a topic-only failure. A field-name change alone never claims a wire-format break. A new overload does not break consumers while their exact baseline signatures remain. Unconsumed removals do not create consumer violations, although `--fail-on warning` can intentionally reject their structural risk.

Suggested remediation is to preserve the original event definition (possibly emitting both old and new events during migration), or coordinate and deploy consumer migrations before adopting a new baseline. ABI declarations cannot confirm that either event is emitted.
