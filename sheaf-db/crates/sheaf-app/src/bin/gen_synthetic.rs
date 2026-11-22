// gen_synthetic.rs - deterministic synthetic benchmark data (C phase).
//
// Timestamp: 2026-11-22_11-53-08 +05:30
// Remote: origin https://github.com/nithin12342/shelf-db.git
// Branch: combined-stack-feature-improvement
// Base commit: a4328dc5890d93cf97a89c26c46a6ecfe33b82d2
//
// The generator writes SQL files only. A fixed xorshift seed makes every run
// byte-identical. Each table stays under the 100,000 row query limit.

use std::fs;
use std::path::PathBuf;

const SEED: u64 = 0x2545_f491_4f6c_dd1d;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, bound: u64) -> u64 {
        self.next() % bound
    }
}

fn main() {
    let dir = PathBuf::from(std::env::args().nth(1).expect("usage: gen_synthetic <dir>"));
    fs::create_dir_all(&dir).expect("create directory");
    let mut rng = Rng(SEED);

    let schema = concat!(
        "CREATE TABLE customer (c_id BIGINT, c_name TEXT, c_segment TEXT, c_balance DOUBLE);\n",
        "CREATE TABLE orders (o_id BIGINT, o_customer BIGINT, o_status TEXT, o_total DOUBLE);\n",
        "CREATE TABLE lineitem (l_id BIGINT, l_order BIGINT, l_part BIGINT, l_quantity DOUBLE, l_price DOUBLE, l_return TEXT);\n",
    );
    fs::write(dir.join("schema.sql"), schema).expect("write schema");

    let mut customer = String::new();
    for id in 1..=100u64 {
        let segment = ["BUILDING", "AUTOMOBILE", "MACHINERY"][(rng.below(3)) as usize];
        let balance = (rng.below(9000) as f64) + 100.0;
        customer.push_str(&format!(
            "INSERT INTO customer VALUES ({id}, 'C{id}', '{segment}', {balance:.2});\n"
        ));
    }
    fs::write(dir.join("load_customer.sql"), customer).expect("write customer");

    let mut orders = String::new();
    for id in 1..=400u64 {
        let customer = 1 + rng.below(100);
        let status = ["F", "O", "P"][(rng.below(3)) as usize];
        let total = (rng.below(400000) as f64) + 1000.0;
        orders.push_str(&format!(
            "INSERT INTO orders VALUES ({id}, {customer}, '{status}', {total:.2});\n"
        ));
    }
    fs::write(dir.join("load_orders.sql"), orders).expect("write orders");

    let mut lineitem = String::new();
    for id in 1..=800u64 {
        let order = 1 + rng.below(400);
        let part = 1 + rng.below(200);
        let quantity = (rng.below(50) as f64) + 1.0;
        let price = (rng.below(900) as f64) + 10.0;
        let marker = if id % 100 == 0 { "R" } else { "A" };
        lineitem.push_str(&format!(
            "INSERT INTO lineitem VALUES ({id}, {order}, {part}, {quantity:.1}, {price:.2}, '{marker}');\n"
        ));
    }
    fs::write(dir.join("load_lineitem.sql"), lineitem).expect("write lineitem");

    println!("synthetic data written to {}", dir.display());
}
