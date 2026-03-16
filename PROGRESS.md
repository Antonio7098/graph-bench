# GraphBench Progress Summary

## What's Working

### Backend (Rust API)
- **Running on**: http://localhost:3001
- **Database**: SQLite with events stored persistently
- **Event streaming**: Events are now stored in DB and can be queried via REST API

### Key Features Implemented

1. **DB-backed Event Storage** (`crates/graphbench-api/src/db.rs`)
   - New `run_events` table stores all events persistently
   - New `runs.status` column tracks run state (running/completed/failed)
   - `GET /api/runs` now includes in-progress runs
   - `GET /api/runs/:id/events` returns events from DB (with fallback to in-memory)

2. **Event Stream** (`crates/graphbench-api/src/event_stream.rs`)
   - Writes events to both in-memory buffer AND database
   - Supports replay for late subscribers

3. **Run Status Tracking**
   - Runs are inserted as "running" when started
   - Updated to "completed" or "failed" when finished

### API Endpoints
- `GET /api/runs` - List all runs (including in-progress)
- `GET /api/runs/:id` - Get run details
- `GET /api/runs/:id/events` - Get events for a run (from DB)
- `POST /api/runs/run` - Start a new benchmark run
- `WS /ws` - WebSocket for live event streaming

## What's Left

### Frontend Updates Needed
1. **Show in-progress runs** in the runs list with visual indicator
2. **Click on in-progress runs** to see live event stream
3. **Add Events tab** to RunDetail view to show event timeline

### Files to Update
- `frontend/src/components/RunList.tsx` - Show status column
- `frontend/src/components/RunDetail.tsx` - Add Events tab
- `frontend/src/api/client.ts` - Add getRunEvents method

## Running

```bash
# Start backend
cargo run --package graphbench-api

# Start frontend
cd frontend && npm run dev
```

## Testing

```bash
# Start a benchmark
curl -X POST http://localhost:3001/api/runs/run \
  -H "Content-Type: application/json" \
  -d '{"task_spec_path": "tasks/prepare-to-edit/task-01.task.json"}'

# Check in-progress runs
curl http://localhost:3001/api/runs

# Get events for a run
curl http://localhost:3001/api/runs/benchmark-xxx/events
```
