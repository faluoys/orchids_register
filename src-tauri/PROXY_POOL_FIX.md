# Proxy Pool Rate Limiting Fix

## Problem
When running batch registrations with proxy pool enabled, the application was hitting rate limits:
- "非法请求：1秒内提取IP数量达到上限" (Rate limit: too many requests within 1 second)
- "非法请求：频率超限：1秒内最多提取10IP" (Frequency limit: max 10 IPs per second)

### Root Cause
Each concurrent registration task created its own `ProxyPool` instance, causing multiple simultaneous API calls that exceeded the proxy API's rate limits (1 request per second).

## Solution

### 1. Added Synchronization to ProxyPool ([src/proxy_pool.rs](src/proxy_pool.rs))

**Changes:**
- Added `#[derive(Clone)]` to `ProxyPool` struct (all fields are already Arc-wrapped, so clones share state)
- Added `fetch_lock: Arc<Mutex<i64>>` field to track last fetch timestamp and prevent concurrent fetches
- Rewrote `get_proxy()` method with proper synchronization:
  - Fast path: Check existing pool first without blocking
  - Acquire fetch_lock to serialize API calls
  - Double-check pool after acquiring lock (another thread may have filled it)
  - Enforce 2-second delay between API calls (with safety margin)
  - Only one thread can fetch at a time, others wait

**Key improvements:**
- Prevents concurrent API calls from multiple threads
- Enforces rate limiting (2 seconds between fetches)
- Threads share proxies from a single fetch (10 proxies per fetch)
- No wasted API calls

### 2. Shared ProxyPool in Batch Registration ([src-tauri/src/commands/register.rs](src-tauri/src/commands/register.rs))

**Changes:**
- Create a single shared `ProxyPool` instance before spawning tasks
- Pass the shared pool to each concurrent registration task
- All tasks use the same pool, preventing duplicate fetches

### 3. Updated Workflow API ([src/workflow.rs](src/workflow.rs))

**Changes:**
- Modified `run_with_args()` signature to accept `Option<ProxyPool>`
- If `Some(pool)` is provided, use it (batch mode)
- If `None`, create a new pool (CLI mode)
- Updated CLI entry point to pass `None`

## How It Works Now

### Batch Registration Flow:
1. User starts batch registration with 10 accounts, concurrency=5
2. System creates ONE shared `ProxyPool` instance
3. Five threads start simultaneously
4. Thread 1 calls `pool.get_proxy()`:
   - Pool is empty
   - Acquires fetch_lock
   - Fetches 10 proxies from API
   - Takes 1 proxy, leaves 9 in pool
   - Releases lock
5. Threads 2-5 call `pool.get_proxy()`:
   - Pool has 9 proxies
   - Each takes 1 proxy (no API call needed)
   - 5 proxies remain
6. When pool depletes, next thread:
   - Acquires fetch_lock
   - Waits 2 seconds (rate limit)
   - Fetches 10 new proxies
   - Continues

### Result:
- Only 1 API call per 10 registrations (instead of 1 per registration)
- No rate limit errors
- Efficient proxy usage
- Proper synchronization

## Testing

To test the fix:
1. Ensure local captcha API is running: `python ..\TurnstileSolver\api_solver.py`
2. Configure freemail settings in the app
3. Start batch registration with proxy pool enabled
4. Monitor logs - should see:
   - "从代理池获取代理: http://x.x.x.x:port" (one per registration)
   - No "非法请求" errors
   - Successful registrations

## Files Modified
- [src/proxy_pool.rs](src/proxy_pool.rs) - Core synchronization logic
- [src/workflow.rs](src/workflow.rs) - Accept shared pool parameter
- [src-tauri/src/commands/register.rs](src-tauri/src/commands/register.rs) - Create and share pool in batch mode

