# WorkClaw Soft Copyright Source Document Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Generate a submission-ready Word source-program document for the full WorkClaw product using the first 30 and last 30 pages of a continuous source listing.

**Architecture:** Build a deterministic source-file manifest, expand it into a continuous line listing, paginate that listing with a fixed per-page source-line budget, and render only the required 60 pages into a `.docx` with A4 layout and monospaced body text. Verify the resulting document from both generator metadata and document-level inspection.

**Tech Stack:** Node.js, `docx` (docx-js), PowerShell, OOXML zip inspection

---

### Task 1: Establish the source selection rules

**Files:**
- Review: `README.md`
- Review: `apps/runtime/src/AGENTS.md`
- Review: `apps/runtime/src-tauri/AGENTS.md`
- Create: `temp/softcopyright/generate-workclaw-source-doc.mjs`

**Step 1: List tracked candidate files**

Run: `git ls-files`
Expected: tracked WorkClaw source files are available for filtering

**Step 2: Define the inclusion groups**

Include only first-party runtime files under:

- `scripts/*.mjs`
- `apps/runtime/src/**/*`
- `apps/runtime/src-tauri/src/**/*`
- `apps/runtime/src-tauri/build.rs`
- `apps/runtime/sidecar/src/**/*`
- `packages/*/src/**/*`

**Step 3: Define exclusion rules**

Exclude vendor, tests, docs, temp outputs, secrets, generated files, and binaries.

### Task 2: Implement the generator

**Files:**
- Create: `temp/softcopyright/generate-workclaw-source-doc.mjs`

**Step 1: Build a deterministic manifest**

Sort by directory group first, then lexicographically within each group.

**Step 2: Expand to a continuous listing**

For each file:

- emit a separator line with the relative path
- emit each source line exactly as text
- normalize tabs to spaces for stable layout

**Step 3: Paginate the full listing**

Use a fixed source-line budget of at least 50 lines per page so page extraction is deterministic.

**Step 4: Emit the `.docx`**

Use:

- A4 page size
- `Courier New` monospaced body text
- exact line spacing sized to keep at least 50 lines per page
- header with software name and document type
- footer with page numbers

### Task 3: Generate and verify the submission document

**Files:**
- Output: `temp/softcopyright/WorkClaw-源程序文档.docx`
- Output: `temp/softcopyright/workclaw-source-doc-metadata.json`

**Step 1: Run the generator**

Run: `node temp/softcopyright/generate-workclaw-source-doc.mjs`
Expected: `.docx` and metadata are created successfully

**Step 2: Inspect generator metadata**

Confirm:

- included file count is non-zero
- total full-listing pages exceed 60
- submission pages equal 60
- source lines per page are at least 50

**Step 3: Inspect the OOXML package**

Confirm:

- A4 page size is set in `word/document.xml`
- header/footer references exist

**Step 4: If available, verify with Word automation**

Open the generated file in Word and confirm the page count reported by Word is 60.

**Step 5: Report the result**

Provide the final file path, the verified page count, the line budget used, and any residual caveats.
