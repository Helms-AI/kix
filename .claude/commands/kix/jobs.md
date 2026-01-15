---
name: kix-jobs
description: |
  Monitor and manage async indexing jobs.

  Usage: /kix-jobs [action] [job-id] [options]
  Examples:
    /kix-jobs                          # List all active jobs
    /kix-jobs abc-123                  # Show specific job status
    /kix-jobs watch abc-123            # Monitor job until complete
    /kix-jobs cancel abc-123           # Cancel a running job
    /kix-jobs history                  # Show recent completed jobs
argument-hint: [list|watch|cancel|history|<job-id>] [--poll N] [--limit N]
---

# KIX Jobs Command

Monitor and manage async indexing jobs for crawling and batch operations.

## Parse Arguments

Extract from `$ARGUMENTS`:

**Actions**:
- **No args or "list"**: List all active/pending jobs
- **<job-id>**: Show detailed status for specific job
- **"watch" <job-id>**: Monitor job until completion with live updates
- **"cancel" <job-id>**: Cancel a running job
- **"history"**: Show recently completed jobs

**Options**:
- **--poll <seconds>**: Polling interval for watch mode (default: 10)
- **--limit <n>**: Maximum jobs to show (default: 20)
- **--all**: Include completed jobs in list
- **--verbose**: Show detailed progress per page

## Action: List Jobs

```
mcp__kix__status with:
- detailed: true
```

Parse the active jobs from status response.

**Output:**
```markdown
## Active Indexing Jobs

| Job ID | Source | Status | Progress | Started |
|--------|--------|--------|----------|---------|
| {short_id} | {url or "batch"} | {status} | {progress}% | {elapsed} ago |
...

**Total**: {active_count} active, {pending_count} pending

**Quick Actions:**
- View details: `/kix-jobs <job-id>`
- Watch progress: `/kix-jobs watch <job-id>`
- Cancel job: `/kix-jobs cancel <job-id>`
```

## Action: Job Details

```
mcp__kix__job_status with:
- job_id: <job-id>
```

**Output:**
```markdown
## Job: {job_id}

### Status: {status}

| Metric | Value |
|--------|-------|
| **Source** | {source_url or source_type} |
| **Progress** | {processed}/{total} ({percentage}%) |
| **Started** | {started_at} |
| **Elapsed** | {elapsed} |
| **ETA** | {eta or "Calculating..."} |

### Progress Bar
{visual progress bar}

### Current Activity
{current_url or current_item}

### Statistics
| Metric | Count |
|--------|-------|
| Pages Crawled | {pages} |
| Documents Created | {documents} |
| Chunks Generated | {chunks} |
| Code Blocks Found | {code_blocks} |
| Errors | {error_count} |

{if errors:}
### Errors
| URL | Error |
|-----|-------|
| {url} | {error} |
...
{/if}

**Actions:**
- Watch: `/kix-jobs watch {job_id}`
- Cancel: `/kix-jobs cancel {job_id}`
```

## Action: Watch Job

Monitor job until completion with periodic updates.

**Execution:**
1. Get initial status
2. Display progress
3. Poll every {poll_interval} seconds
4. Update display with new progress
5. Continue until job completes or is cancelled

**Output (live updating):**
```markdown
## Watching Job: {job_id}

**Status**: {status}
**Progress**: [{progress_bar}] {percentage}%
**Current**: {current_url}
**ETA**: {eta}

### Activity Log
{timestamp} - {activity}
{timestamp} - {activity}
...

Press Ctrl+C to stop watching (job continues in background)
```

**On Completion:**
```markdown
## Job Complete: {job_id}

### Final Results
- **Duration**: {total_time}
- **Status**: {success ? "Success" : "Completed with errors"}
- **Documents Created**: {documents}
- **Chunks Generated**: {chunks}
- **Code Blocks Extracted**: {code_blocks}

{if errors:}
### Errors Encountered
| URL | Error |
|-----|-------|
| {url} | {error} |
{/if}

### Quality Summary
| Metric | Value |
|--------|-------|
| Avg Chunks/Doc | {avg} |
| Code Extraction Rate | {rate}% |
| Duplicate Pages Skipped | {skipped} |

**Next Steps:**
- Search new content: `/kix-search "<topic>"`
- Check overall status: `/kix-status`
- Link to project: `/kix-link <project> --search "<topic>"`
```

## Action: Cancel Job

```
# Note: Cancellation requires MCP tool support
# If mcp__kix__cancel_job exists, use it
# Otherwise, inform user that job cannot be cancelled
```

**Output:**
```markdown
## Job Cancellation: {job_id}

{if cancelled:}
- **Status**: Cancelled
- **Progress at Cancellation**: {percentage}%
- **Documents Indexed Before Cancel**: {documents}

Note: Already indexed content is preserved.
{else:}
Job cancellation is not supported for this job type.
The job will continue running in the background.
{/if}
```

## Action: History

Show recently completed jobs.

**Output:**
```markdown
## Recent Job History

| Job ID | Source | Status | Duration | Documents | Completed |
|--------|--------|--------|----------|-----------|-----------|
| {id} | {source} | {status} | {duration} | {docs} | {time_ago} |
...

**Totals (last 24h)**:
- Jobs Completed: {count}
- Documents Indexed: {total_docs}
- Success Rate: {rate}%

View details: `/kix-jobs <job-id>`
```

## Integration with Workflows

The workflow orchestrator uses this skill for the `4_monitor` phase:

```yaml
4_monitor:
  skill: /kix-jobs
  args: 'watch {job_id} --poll 30'
  purpose: "Monitor crawl progress"
  until: job_status == "completed"
```

For batch monitoring (multiple jobs):

```markdown
## Monitoring {n} Jobs

| Job | Source | Progress | Status |
|-----|--------|----------|--------|
| 1 | {url1} | {prog1}% | {status1} |
| 2 | {url2} | {prog2}% | {status2} |
...

**Overall Progress**: {total_processed}/{total_items} ({overall_pct}%)
**Estimated Completion**: {eta}
```

## Polling Best Practices

- **Short jobs (<100 pages)**: Poll every 5-10 seconds
- **Medium jobs (100-500 pages)**: Poll every 15-30 seconds
- **Large jobs (>500 pages)**: Poll every 30-60 seconds
- **Batch jobs**: Poll every 60 seconds, show aggregate progress

## Error Handling

- **Job not found**: Suggest checking `/kix-jobs history`
- **Job already completed**: Show final results
- **Connection timeout**: Retry poll, inform user
- **Multiple jobs with same prefix**: List matches, ask for full ID
