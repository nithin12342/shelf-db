"""check_table_values.py - print every report value from the result files.

The tags below are the pass recorded in the report. Each printed line is read
from the JSON file, not typed by hand.
"""
import json
import os

RES = os.path.join(os.path.dirname(os.path.abspath(__file__)), "results")
TAGS = {
    "pg": "run14_20261026",
    "mariadb": "run16_20261026",
    "redis": "run16_20261026",
    "duckdb": "run1_20261026",
    "sqlite": "run1_20261026",
    "neo4j": "run1_20261026",
    "scylladb": "run1_20261026",
}


def load(name):
    return json.load(open(os.path.join(RES, name + ".json")))


def main():
    print("engine    scan_ms  join_ms   reps  p50_us    p99_us    lookups")
    for eng, tag in TAGS.items():
        r = load(eng)[tag]
        j = r.get("join_ms", {})
        p = r["point_lookup_us"]
        print(f"{eng:9s} {str(r.get('scan_agg_ms', {}).get('p50')):8s} "
              f"{str(j.get('p50')):9s} {str(j.get('n')):5s} "
              f"{p['p50']:8.1f}  {p['p99']:8.1f}  {p['n']}")

    n = load("neo4j")["run1_20261026"]
    print("neo4j graph: full", n["full_traversal_ms"]["p50"],
          "paths", n["full_traversal_ms"]["paths"],
          "two_hop", n["two_hop_us"]["p50"])

    j = load("imdb_job")
    print("imdb_job:", {k: (v["duckdb_ms"], v["pg_ms"], v["rows"])
                        for k, v in j["queries"].items()})

    m = load("imdb_multi_engine")
    tag = "run_20261026_1423_detail"
    print("imdb_multi", tag, {k: round(v["ms"], 1) for k, v in m[tag].items()
                              if isinstance(v, dict)})

    s = load("imdb_shelf")
    print("imdb_shelf: q1", s["q1"]["load_ms"], s["q1"]["query_ms"],
          "q2", s["q2"]["load_ms"], s["q2"]["query_ms"],
          "q3", s["q3"]["load_ms"], s["q3"]["query_ms"],
          "q4", s["q4"]["load_ms"], s["q4"]["query_ms"],
          "oracle", s["oracle_ms"])

    q = load("shelf_bins_20261026")
    print("shared pass: scan", q["tpch_scan_bench"]["rep_ms"],
          "fused", q["fused_scan_bench"]["best_ms"],
          "join", q["fiber_join_bench"]["load_ms"],
          q["fiber_join_bench"]["query_ms"], q["fiber_join_bench"]["total_ms"],
          "graph", q["graph_bench"]["full_ms"],
          q["graph_bench"]["sampled_2hop_p50_us"])


if __name__ == "__main__":
    main()
