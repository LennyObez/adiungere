## What changed

<!-- One or two sentences. What a reader needs to know before looking at the diff. -->

## What proves it

<!-- The test that fails without this change, and the command that runs it. A result without the command that
     produced it is not a result. -->

## Checklist

- [ ] A test fails without this change, and I ran it in the failing state first
- [ ] The test asserts observable behaviour, not the steps taken
- [ ] The whole gate sequence passed locally, not a scoped run
- [ ] No finding was silenced with an allow attribute, an exclusion or an ignore entry
- [ ] Nothing here sends a recording anywhere, or adds a request that leaves the device
- [ ] Nothing here opens a source recording for writing
- [ ] No claim about authenticity was added, and no status reads as a verdict
- [ ] Any export path this touches still preserves the vendor telemetry byte for byte
- [ ] No user-visible string was added outside the wording catalogue
- [ ] Nothing here names a third-party product where a generic description would do
- [ ] No tracked file carries a path, a host or a tool from a development environment
- [ ] A decision record accompanies any change to a boundary, a dependency or a guarantee
- [ ] A probe that this change relies on has a verdict in the evidence register
