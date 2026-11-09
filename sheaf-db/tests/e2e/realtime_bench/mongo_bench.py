"""MongoDB E2E bench: 5 collections x 100k docs, $lookup pipeline, mixed R/W.
Usage: py mongo_bench.py   (requires the configured MongoDB server)
"""
import json
import os
import random
import time
from concurrent.futures import ThreadPoolExecutor

from pymongo import ASCENDING, MongoClient

import bench_paths as P

OUT = str(P.RESULTS / "mongodb.json")
SEED = 42
N = 100_000


def pct(times, p):
    s = sorted(times)
    return s[min(len(s) - 1, int(p * len(s)))]


def main():
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    rng = random.Random(SEED)
    mongo = P.service("mongo")
    cli = MongoClient(mongo.host, mongo.port, serverSelectionTimeoutMS=10000)
    res = {"mongo_version": cli.server_info()["version"]}
    db = cli.bench
    if db.payments.estimated_document_count() == N:
        print(f"already loaded ({N}/collection), skipping")
    else:
        for coll in ["customers", "orders", "items", "payments", "shipments"]:
            db[coll].drop()
        a = time.perf_counter()
        regions = ["na", "eu", "apac", "latam"]
        cats = ["grocery", "travel", "electronics", "fuel", "dining"]
        db.customers.insert_many([{"_id": i, "name": f"u{i}", "region": regions[i % 4]} for i in range(1, N + 1)], ordered=False)
        db.items.insert_many([{"_id": i, "price": round(5 + (i % 5000) / 100, 2), "cat": cats[i % 5]} for i in range(1, N + 1)], ordered=False)
        db.orders.insert_many([{"_id": i, "cust_id": (i % N) + 1, "item_ids": [(i % N) + 1, ((i + 7) % N) + 1], "total": round(10 + (i % 9000) / 100, 2)} for i in range(1, N + 1)], ordered=False)
        db.payments.insert_many([{"_id": i, "order_id": i, "amount": round(10 + (i % 9000) / 100, 2), "method": "card" if i % 2 else "ach"} for i in range(1, N + 1)], ordered=False)
        db.shipments.insert_many([{"_id": i, "order_id": i, "carrier": "ups" if i % 2 else "fedex", "zone": i % 9} for i in range(1, N + 1)], ordered=False)
        res["load_s"] = round(time.perf_counter() - a, 1)
        print(f"loaded in {res['load_s']}s")
    for coll, key in (("payments", "order_id"), ("orders", "cust_id"), ("shipments", "order_id")):
        try:
            db[coll].drop_index([(key, ASCENDING)])
        except Exception:
            pass
    counts = {c: db[c].estimated_document_count() for c in ["customers", "orders", "items", "payments", "shipments"]}
    assert all(v == N for v in counts.values()), counts
    res["counts"] = counts

    # 5.1: 4-stage $lookup pipeline (payment -> order -> customer + items + shipment)
    pipe = [
        {"$match": {"_id": {"$gte": 1, "$lte": 200}}},
        {"$lookup": {"from": "orders", "localField": "order_id", "foreignField": "_id", "as": "o"}},
        {"$unwind": "$o"},
        {"$lookup": {"from": "customers", "localField": "o.cust_id", "foreignField": "_id", "as": "c"}},
        {"$unwind": "$c"},
        {"$lookup": {"from": "shipments", "localField": "order_id", "foreignField": "order_id", "as": "s"}},
        {"$unwind": "$s"},
        {"$project": {"amount": 1, "c.name": 1, "o.total": 1, "s.carrier": 1}},
    ]
    ts = []
    for _ in range(5):
        a = time.perf_counter_ns()
        rows = list(db.payments.aggregate(pipe))
        ts.append((time.perf_counter_ns() - a) / 1e6)
    res["lookup4_unindexed_ms"] = {"p50": round(pct(ts, 0.5), 1), "rows": len(rows), "n": len(ts)}

    # 5.1b: same pipeline with all lookup indexes present (truthful indexed pair).
    db.payments.create_index([("order_id", ASCENDING)])
    db.orders.create_index([("cust_id", ASCENDING)])
    db.shipments.create_index([("order_id", ASCENDING)])
    ts = []
    for _ in range(5):
        a = time.perf_counter_ns()
        rows = list(db.payments.aggregate(pipe))
        ts.append((time.perf_counter_ns() - a) / 1e6)
    res["lookup4_indexed_ms"] = {"p50": round(pct(ts, 0.5), 1), "rows": len(rows), "n": len(ts)}

    # 5.2: mixed 50/50 read/write over 32 threads (laptop-scaled from 128; recorded)
    def worker(seed):
        r = random.Random(seed)
        t = []
        c = MongoClient(mongo.host, mongo.port)
        d = c.bench
        for _ in range(500):
            i = r.randint(1, N)
            if r.random() < 0.5:
                a = time.perf_counter_ns()
                d.payments.find_one({"_id": i})
                t.append((time.perf_counter_ns() - a) / 1000.0)
            else:
                a = time.perf_counter_ns()
                d.payments.update_one({"_id": i}, {"$set": {"method": "card"}})
                t.append((time.perf_counter_ns() - a) / 1000.0)
        return t

    a = time.perf_counter()
    with ThreadPoolExecutor(max_workers=32) as ex:
        nested = list(ex.map(worker, range(32)))
    wall = time.perf_counter() - a
    flat = [x for sub in nested for x in sub]
    res["mixed_32t"] = {"ops": len(flat), "wall_s": round(wall, 1),
                        "throughput": round(len(flat) / wall, 0),
                        "p50_us": round(pct(flat, 0.5), 1), "p99_us": round(pct(flat, 0.99), 1)}

    with open(OUT, "w") as f:
        json.dump(res, f, indent=2)
    print(json.dumps(res, indent=2))
    print("SAVED", OUT)


if __name__ == "__main__":
    main()
