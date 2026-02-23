---
name: release
description: Cut a new zelligent release — bump version, tag, push, wait for CI, update GitHub release notes, and update the Homebrew formula
allowed-tools: Bash, Read, Edit, Grep
---

## Release process

### 1. Bump VERSION

Read `VERSION`, increment the patch number (or as instructed), and write it back.

### 2. Commit, tag, and push

```bash
git add VERSION
git commit -m "Bump version to <version>"
git tag v<version>
git push origin main --follow-tags
git push origin v<version>
```

If the tag already exists on the remote (from a previous failed attempt), delete it first:

```bash
gh release delete v<version> --yes 2>/dev/null
git push origin :refs/tags/v<version>
git push origin v<version>
```

### 3. Wait for the release CI

```bash
gh run watch $(gh run list --workflow=release.yml --limit 1 --json databaseId -q '.[0].databaseId') --exit-status
```

Timeout: 5 minutes.

### 4. Update GitHub release notes

Get the SHA256 from the release body:

```bash
gh release view v<version> --json body -q .body | head -1
```

Get the list of PRs merged since the last release:

```bash
gh pr list --state merged --search "merged:>$(gh release view <prev-version> --json publishedAt -q .publishedAt | cut -dT -f1)" --json number,title | jq -r '.[] | "- \(.title) (#\(.number))"'
```

Then update the release body with `gh release edit v<version> --notes "..."`. Follow the format of previous releases:

```
SHA256: `<sha256>`

## Install

\`\`\`
brew install pcomans/zelligent/zelligent
zelligent doctor
\`\`\`

## What's Changed

<categorized list of changes>

**Full Changelog**: https://github.com/pcomans/zelligent/compare/<prev-version>...v<version>
```

### 5. Update the Homebrew formula

The Homebrew tap repo is at `/Users/philipp/code/homebrew-zelligent/`.

1. `cd /Users/philipp/code/homebrew-zelligent && git pull`
2. Edit `Formula/zelligent.rb`: update `version` and `sha256`
3. Commit and push:

```bash
cd /Users/philipp/code/homebrew-zelligent && git add Formula/zelligent.rb && git commit -m "Update zelligent to v<version>" && git push
```

### 6. Verify

Confirm the release is live:

```bash
gh release view v<version> --json tagName,name -q '.tagName'
```
