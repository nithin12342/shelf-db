"""Redis E2E bench: 1M KV pairs, point lookups, Lua consistency check.
Usage: py redis_bench.py   (requires the configured Redis server)
"""
import json
import os
import random
import time

import redis

import bench_paths as P

OUT = str(P.RESULTS / "redis.json")
SEED = 42
NKEYS = 1_000_000


def pct(times, p):
    s = sorted(times)
    return s[min(len(s) - 1, int(p * len(s)))]


def main():
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    redis_svc = P.service("redis")
    r = redis.Redis(host=redis_svc.host, port=redis_svc.port, socket_timeout=60)
    r.ping()
    res = {"redis_version": r.info()["redis_version"]}
    if r.dbsize() == NKEYS:
        print(f"already loaded ({NKEYS}), skipping")
    else:
        r.flushdb()
        a = time.perf_counter()
        pipe = r.pipeline(transaction=False)
        for i in range(1, NKEYS + 1):
            pipe.hset(f"acct:{i}", mapping={"bal": str(1000 + (i % 9000)), "name": f"u{i}", "pad": "x" * 8})
            if i % 10000 == 0:
                pipe.execute()
                pipe = r.pipeline(transaction=False)
        pipe.execute()
        res["load_s"] = round(time.perf_counter() - a, 1)
        print(f"loaded in {res['load_s']}s")
    assert r.dbsize() == NKEYS, r.dbsize()
    res["dbsize"] = r.dbsize()
    res["used_memory_mb"] = round(r.info("memory")["used_memory"] / 1e6, 1)

    rng = random.Random(SEED)
    keys = rng.sample(range(1, NKEYS + 1), 20000)
    ts = []
    for k in keys:
        a = time.perf_counter_ns()
        r.hgetall(f"acct:{k}")
        ts.append((time.perf_counter_ns() - a) / 1000.0)
    res["point_lookup_us"] = {"p50": round(pct(ts, 0.5), 1), "p99": round(pct(ts, 0.99), 1), "n": len(ts)}

    lua = """
    local sum = 0
    for i, k in ipairs(KEYS) do
      sum = sum + tonumber(redis.call('HGET', k, 'bal'))
    end
    return sum
    """
    check = r.register_script(lua)
    groups = [[f"acct:{rng.randint(1, NKEYS)}" for _ in range(5)] for _ in range(2000)]
    ts = []
    for g in groups:
        a = time.perf_counter_ns()
        check(keys=g)
        ts.append((time.perf_counter_ns() - a) / 1000.0)
    res["lua_5key_us"] = {"p50": round(pct(ts, 0.5), 1), "p99": round(pct(ts, 0.99), 1), "n": len(ts)}

    with open(OUT, "w") as f:
        json.dump(res, f, indent=2)
    print(json.dumps(res, indent=2))
    print("SAVED", OUT)


if __name__ == "__main__":
    main()
