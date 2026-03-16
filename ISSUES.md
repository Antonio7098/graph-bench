# GraphBench API - Current Issues Summary

## What We Built

### Backend (Rust API)
- **Location**: `crates/graphbench-api/`
- **Running on**: http://localhost:3001

### Features Implemented
1. **DB-backed Event Storage** - Events stored in SQLite `run_events` table
2. **Run Status Tracking** - Runs have `status: "running"|"completed"|"failed"`
3. **REST API Endpoints**:
   - `GET /api/runs` - List all runs (including in-progress)
   - `GET /api/runs/:id` - Get run details
   - `GET /api/runs/:id/events` - Get events from DB
   - `POST /api/runs/run` - Start benchmark
   - `WS /ws` - WebSocket for live streaming

## The Problem

### Issue: Benchmarks Hang at "model.request_sent"

When running a benchmark via the API, it:
1. ✅ Starts successfully
2. ✅ Loads fixture, workspace, task
3. ✅ Registers tools
4. ✅ Sends model request to OpenRouter
5. ❌ **HANGS** - never receives response

The last event is `model.request_sent` but no `model.response_received` ever comes.

### Root Cause

The harness library (`graphbench-harness`) makes **synchronous HTTP calls** to OpenRouter using the `llm` crate's client. When we run this inside `tokio::spawn` or any async context, it blocks the async runtime and causes issues.

We tried:
1. `tokio::spawn(async { ... })` - blocks the runtime
2. `std::thread::spawn` with `tokio::runtime::Builder::new_current_thread()` - "Cannot start runtime from within runtime"
3. Direct blocking call in spawned thread - currently not working

### It Was Working Before

The original implementation spawned a **subprocess** (the `smoke_openrouter` binary) which ran independently. That worked because it was a separate process.

Now we're calling the harness library **directly** and it doesn't work.

## What Was Working Before

The original flow:
1. API receives POST /api/runs/run
2. Spawns subprocess: `cargo run --package graphbench-harness --bin smoke_openrouter`
3. Subprocess runs independently, writes to traces/
4. API imports traces when done

## Current Code State

### api.rs - start_run function
Currently uses `std::thread::spawn` but calls `run_benchmark_sync` which doesn't exist yet.

### harness.rs
- Has `pub async fn run_benchmark()` - async version
- Needs `pub fn run_benchmark_sync()` - synchronous version that can run in a blocking thread

## What Needs To Be Fixed

1. **Create synchronous harness runner** - A version of the harness that works in a blocking context
2. **Or revert to subprocess** - Spawn the binary again instead of calling library directly
3. **Add timeout** - The model request should timeout after ~60s so we get an error instead of hanging forever

## Files Involved

- `crates/graphbench-api/src/api.rs` - HTTP handlers
- `crates/graphbench-api/src/harness.rs` - Harness integration (BROKEN)
- `crates/graphbench-api/src/db.rs` - Database operations (WORKING)
- `crates/graphbench-api/src/event_stream.rs` - Event streaming (WORKING)
- `crates/graphbench-harness/src/bin/smoke_openrouter.rs` - Working binary

## Quick Fix Options

### Option 1: Revert to subprocess (easiest)
Go back to spawning `smoke_openrouter` as a child process. Works but loses direct integration.

### Option 2: Make harness truly synchronous
Rewrite harness to use blocking HTTP client (reqwest blocking) instead of async.

### Option 3: Add proper timeout
At minimum, add a 60s timeout to the model request so it fails fast instead of hanging forever.
