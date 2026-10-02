#!/usr/bin/env bash
# gen-stranger.sh <dir> — `tidepool`, an invented job queue with one finished
# feature, the retry policy, and no documentation and no docsys: the
# stranger test (backlog M3). Nothing in the repository says how docsys binds
# a page to code — no `doc:` line anywhere — so whatever the session knows of
# it, it learned from docsys itself.
source "$(dirname "${BASH_SOURCE[0]}")/../lib.sh"
lab_binary
DIR=${1:?usage: gen-stranger.sh <dir>}
[ -e "$DIR" ] && fail "$DIR exists"
lab_git_init "$DIR"
cd "$DIR"

mkdir -p src
printf '# tidepool\n\nA small job queue for field devices. Invented for the docsys agent lab.\n' > README.md
printf '{\n  "name": "tidepool",\n  "version": "0.3.0",\n  "private": true\n}\n' > package.json
printf 'node_modules/\n' > .gitignore
cat > src/queue.ts <<'TS'
import { delayFor, shouldRetry } from "./retry"

export interface Job { id: string; attempt: number; run(): Promise<number> }

export class Queue {
  private jobs: Job[] = []
  push(job: Job) { this.jobs.push(job) }
  async drain(sleep: (s: number) => Promise<void>) {
    for (const job of this.jobs.splice(0)) {
      let status = await job.run()
      while (status >= 400 && shouldRetry(job.attempt, status)) {
        job.attempt += 1
        await sleep(delayFor(job.attempt))
        status = await job.run()
      }
    }
  }
}
TS
dated_commit . 2025-01-10 "feat: a queue that drains jobs in order"

cat > src/retry.ts <<'TS'
// Retries back off exponentially from 2 seconds and stop after 6 attempts:
// the longest outage measured in the field (2025-03-02) lasted 58 seconds, and
// 2 + 4 + 8 + 16 + 32 = 62 seconds of waiting covers it.
export const FIRST_DELAY_SECONDS = 2
export const MAX_ATTEMPTS = 6

export function delayFor(attempt: number): number {
  return FIRST_DELAY_SECONDS * 2 ** (attempt - 1)
}

// A 4xx fails the same way again: the job is wrong, not the network.
export function shouldRetry(attempt: number, status: number): boolean {
  if (status >= 400 && status < 500) return false
  return attempt < MAX_ATTEMPTS
}
TS
dated_commit . 2025-03-05 "feat: retry failed jobs with backoff" "The 2025-03-02 outage lasted 58 seconds; six attempts from 2 seconds cover 62. Client errors (4xx) are never retried."
dated_commit . 2025-03-06 "chore: bump version"
