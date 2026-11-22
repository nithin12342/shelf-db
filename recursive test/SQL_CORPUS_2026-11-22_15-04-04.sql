-- SQL corpus from COMPLETE_EXECUTION_PLAN_2026-11-22_09-02-56.md
-- Timestamp: 2026-11-22_15-04-04 +05:30
-- Remote: origin https://github.com/nithin12342/shelf-db.git
-- Branch: combined-stack-feature-improvement
-- Top commit at writing: 8628c2c799f361e98fe11b00ec454ab682c40303

Q01: SELECT r_name FROM region WHERE r_regionkey = 2;
Q02: SELECT c_name FROM customer WHERE c_custkey = 42;
Q03: SELECT c_custkey, c_acctbal FROM customer WHERE c_acctbal > 9000;
Q04: SELECT o_orderkey FROM orders WHERE o_totalprice BETWEEN 100000 AND 200000;
Q05: SELECT COUNT(*) FROM lineitem WHERE l_quantity < 5;
Q06: SELECT SUM(l_extendedprice) FROM lineitem;
Q07: SELECT MIN(o_totalprice), MAX(o_totalprice) FROM orders;
Q08: SELECT AVG(c_acctbal) FROM customer;
Q09: SELECT l_returnflag, SUM(l_extendedprice) FROM lineitem GROUP BY l_returnflag;
Q10: SELECT c_mktsegment, COUNT(*) FROM customer GROUP BY c_mktsegment ORDER BY c_mktsegment;
Q11: SELECT DISTINCT l_returnflag FROM lineitem;
Q12: SELECT COUNT(*) FROM lineitem WHERE l_returnflag IS NULL;
Q13: SELECT COUNT(*) FROM customer WHERE c_active = TRUE;
Q14: SELECT o_orderkey, o_totalprice FROM orders ORDER BY o_totalprice DESC LIMIT 20;
Q15: SELECT o.o_orderkey, c.c_name FROM orders o INNER JOIN customer c ON o.o_custkey = c.c_custkey WHERE o.o_totalprice > 400000;
Q16: SELECT l.l_orderkey, p.p_name FROM lineitem l INNER JOIN part p ON l.l_partkey = p.p_partkey WHERE l.l_quantity > 40;
Q17: SELECT s.s_name, ps.ps_supplycost FROM partsupp ps INNER JOIN supplier s ON ps.ps_suppkey = s.s_suppkey WHERE ps.ps_availqty > 5000;
Q18: SELECT n.n_name, SUM(o.o_totalprice) FROM orders o INNER JOIN customer c ON o.o_custkey = c.c_custkey INNER JOIN nation n ON c.c_nationkey = n.n_nationkey GROUP BY n.n_name;
Q19: SELECT r.r_name, COUNT(*) FROM customer c INNER JOIN nation n ON c.c_nationkey = n.n_nationkey INNER JOIN region r ON n.n_regionkey = r.r_regionkey GROUP BY r.r_name;
Q20: SELECT c.c_name, o.o_totalprice FROM customer c INNER JOIN orders o ON c.c_custkey = o.o_custkey ORDER BY o.o_totalprice DESC LIMIT 10;
Q21: SELECT l.l_orderkey FROM lineitem l INNER JOIN orders o ON l.l_orderkey = o.o_orderkey WHERE o.o_orderpriority = '1-URGENT';
Q22: SELECT p_name FROM part WHERE p_brand = 'Brand#12';
Q23: SELECT COUNT(*) FROM orders o INNER JOIN customer c ON o.o_custkey = c.c_custkey;
Q24: SELECT COUNT(*) FROM lineitem;
