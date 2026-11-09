"""One-shot: add missing shipments index, re-run $lookup, update mongodb.json."""
import json
import time

from pymongo import ASCENDING, MongoClient

import bench_paths as P

OUT = str(P.RESULTS / "mongodb.json")

mongo = P.service("mongo")
cli = MongoClient(mongo.host, mongo.port)
db = cli.bench
db.shipments.create_index([("order_id", ASCENDING)])
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
s = sorted(ts)
print("indexed lookup4 ms:", [round(x, 1) for x in s])
with open(OUT) as f:
    d = json.load(f)
d["lookup4_unindexed_ms"] = d.pop("lookup4_ms")
d["lookup4_indexed_ms"] = {"p50": round(s[2], 1), "rows": len(rows), "n": 5}
with open(OUT, "w") as f:
    json.dump(d, f, indent=2)
print("JSON updated")
