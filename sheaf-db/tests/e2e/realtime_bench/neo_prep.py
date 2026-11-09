"""One-shot: TPC-H SF1 -> Neo4j bulk-import CSVs (subset columns for speed)."""
import os

import bench_paths as P

SRC = P.build("tpch-dbgen")
DST = P.data("neo4j", "import")
os.makedirs(DST, exist_ok=True)

with open(f"{SRC}\\customer.tbl", encoding="utf-8") as f, \
     open(f"{DST}\\cust.csv", "w", encoding="utf-8") as o:
    o.write("custkey:ID(Customer),name,acctbal\n")
    for line in f:
        p = line.rstrip("\n").split("|")
        o.write(f"{p[0]},{p[1]},{p[5]}\n")

with open(f"{SRC}\\orders.tbl", encoding="utf-8") as f, \
     open(f"{DST}\\ord.csv", "w", encoding="utf-8") as o, \
     open(f"{DST}\\placed.csv", "w", encoding="utf-8") as e:
    o.write("orderkey:ID(Order),totalprice\n")
    e.write(":START_ID(Customer),:END_ID(Order)\n")
    for line in f:
        p = line.rstrip("\n").split("|")
        o.write(f"{p[0]},{p[3]}\n")
        e.write(f"{p[1]},{p[0]}\n")

with open(f"{SRC}\\lineitem.tbl", encoding="utf-8") as f, \
     open(f"{DST}\\line.csv", "w", encoding="utf-8") as o, \
     open(f"{DST}\\contains.csv", "w", encoding="utf-8") as e:
    o.write("lineid:ID(Line),price\n")
    e.write(":START_ID(Order),:END_ID(Line)\n")
    n_lines = 0
    for line in f:
        if n_lines >= 600000:
            break  # slice: first 600k lineitem rows (orderkeys always reference existing orders)
        p = line.rstrip("\n").split("|")
        lid = int(p[0]) * 10 + int(p[3])
        o.write(f"{lid},{p[5]}\n")
        e.write(f"{p[0]},{lid}\n")
        n_lines += 1
print("CSV prep done")
