# Hash Joins — "Two tables walk into a HashMap"

Your database has two tables: `orders` with 10 million rows and `customers` with 100,000 rows. A user runs `SELECT * FROM orders JOIN customers ON orders.customer_id = customers.id`. For every order, you need to find the matching customer. The obvious approach -- for each order, scan all customers -- means 10 million times 100,000 comparisons. That is one trillion comparisons. Your query will finish sometime next week.

The hash join brings that down to roughly 10 million comparisons. One per order. Let's build one from scratch and understand why.

---

## The Naive Way

The simplest join strategy is the **nested loop join**. For each row in the left table, scan the entire right table looking for matches:

```rust
fn main() {
    // Simulate two tables
    let customers: Vec<(u32, String)> = (0..1_000)
        .map(|i| (i, format!("Customer_{}", i)))
        .collect();

    let orders: Vec<(u32, u32, f64)> = (0..10_000)
        .map(|i| (i, i % 1_000, 9.99 + i as f64)) // (order_id, customer_id, amount)
        .collect();

    // Nested loop join: for each order, scan all customers
    let mut matches = 0u64;
    let mut comparisons = 0u64;

    for (_order_id, cust_id, _amount) in &orders {
        for (id, _name) in &customers {
            comparisons += 1;
            if cust_id == id {
                matches += 1;
                break; // found the match, stop scanning
            }
        }
    }

    println!("Matches found: {}", matches);
    println!("Total comparisons: {}", comparisons);
    println!("Average comparisons per order: {}", comparisons / orders.len() as u64);

    // With 10,000 orders and 1,000 customers:
    // Average ~500 comparisons per order (scanning half the customer list)
    // Total: ~5,000,000 comparisons
    // Scale to 10M orders x 100K customers = 500 billion comparisons
}
```

Five million comparisons for just 10,000 orders against 1,000 customers. Scale that to real table sizes and the numbers become absurd. The nested loop join is O(n * m) where n and m are the sizes of the two tables. Every row in the outer table triggers a full scan of the inner table.

The fundamental problem: we are redoing the same linear search over and over. For every order, we walk through customers from the beginning, looking for a matching ID. We never remember where we found things.

---

## The Insight

Imagine you are organizing a school reunion. You have two lists: a list of 10,000 RSVPs (each with a class year) and a list of 50 class photos (each labeled with a year). You need to match each RSVP to their class photo.

The slow way: pick up each RSVP, flip through all 50 photos until you find the matching year. That is 10,000 times 50 -- half a million look-throughs.

The fast way: first, spread the 50 photos on a table, organized by year. Photo for 1998 goes in the 1998 spot. Photo for 2003 goes in the 2003 spot. This takes one pass through 50 photos. Now pick up each RSVP, glance at the year, and reach directly for the right photo. One look per RSVP. That is 50 setup steps plus 10,000 lookups -- roughly 10,050 operations instead of 500,000.

That is a hash join. It has two phases:

1. **Build phase**: hash the smaller table into a HashMap. One pass, O(m) time.
2. **Probe phase**: for each row of the larger table, look up the join key in the HashMap. One pass, O(n) time.

Total: O(n + m) instead of O(n * m). For 10 million orders and 100,000 customers, that is 10.1 million operations instead of one trillion.

The critical optimization: always build the hash table on the **smaller** table. The hash table must fit in memory. If you hash 100,000 customers, that is maybe 10 MB of RAM. If you tried to hash 10 million orders, you would need 1 GB or more. Same result, vastly different memory usage.

---

## The Build

### The Join Key and Row Types

First, let's define our data. In a real database, rows are tuples of columns. We will keep it simple -- a customer has an ID and a name, an order has an ID, a customer ID (the join key), and an amount:

```rust,ignore
#[derive(Debug, Clone)]
struct Customer {
    id: u32,
    name: String,
}

#[derive(Debug, Clone)]
struct Order {
    id: u32,
    customer_id: u32,
    amount: f64,
}

#[derive(Debug, Clone)]
struct JoinedRow {
    order_id: u32,
    customer_id: u32,
    amount: f64,
    customer_name: String,
}
```

### The Build Phase

The build phase takes the smaller table and inserts every row into a HashMap, keyed by the join column. A critical detail: multiple rows might share the same key. Two customers could have the same ID in a denormalized dataset, or more commonly, we might join on a non-unique column. So the HashMap maps each key to a **Vec** of rows:

```rust,ignore
use std::collections::HashMap;

fn build_hash_table(customers: &[Customer]) -> HashMap<u32, Vec<&Customer>> {
    let mut table: HashMap<u32, Vec<&Customer>> = HashMap::with_capacity(customers.len());

    for customer in customers {
        table.entry(customer.id).or_insert_with(Vec::new).push(customer);
    }

    table
}
```

We use `HashMap::with_capacity` to avoid repeated resizing. We know exactly how many entries we will insert, so we allocate upfront. This avoids the O(n) rehash cost that would otherwise trigger multiple times during insertion.

### The Probe Phase

The probe phase walks through the larger table. For each row, it computes the join key, looks it up in the hash table, and emits a joined row for each match:

```rust,ignore
fn probe(orders: &[Order], hash_table: &HashMap<u32, Vec<&Customer>>) -> Vec<JoinedRow> {
    let mut results = Vec::new();

    for order in orders {
        if let Some(matching_customers) = hash_table.get(&order.customer_id) {
            for customer in matching_customers {
                results.push(JoinedRow {
                    order_id: order.id,
                    customer_id: order.customer_id,
                    amount: order.amount,
                    customer_name: customer.name.clone(),
                });
            }
        }
        // If no match in the hash table, this order is skipped (inner join semantics)
    }

    results
}
```

Notice the inner loop over `matching_customers`. If three customers share the same ID, one order produces three joined rows. This is correct -- it is how SQL joins work. The Cartesian product of matching rows.

### Putting It Together: The HashJoinIterator

In a real query engine, joins are lazy -- they produce rows one at a time using the iterator (Volcano) model. Let's build a proper iterator that yields joined rows on demand instead of materializing the entire result:

```rust,ignore
struct HashJoinIterator<'a> {
    // Build side: the hash table from the smaller input
    hash_table: HashMap<u32, Vec<&'a Customer>>,
    // Probe side: the larger input
    orders: &'a [Order],
    // Current position in the probe input
    probe_index: usize,
    // When a probe row matches multiple build rows, we need to
    // iterate through them. This tracks our position within the
    // current match set.
    match_buffer: Vec<JoinedRow>,
    match_index: usize,
}

impl<'a> HashJoinIterator<'a> {
    fn new(customers: &'a [Customer], orders: &'a [Order]) -> Self {
        // Build phase happens eagerly in the constructor
        let mut hash_table: HashMap<u32, Vec<&'a Customer>> =
            HashMap::with_capacity(customers.len());

        for customer in customers {
            hash_table.entry(customer.id).or_insert_with(Vec::new).push(customer);
        }

        HashJoinIterator {
            hash_table,
            orders,
            probe_index: 0,
            match_buffer: Vec::new(),
            match_index: 0,
        }
    }
}

impl<'a> Iterator for HashJoinIterator<'a> {
    type Item = JoinedRow;

    fn next(&mut self) -> Option<JoinedRow> {
        loop {
            // If we have buffered matches from the current probe row, yield them
            if self.match_index < self.match_buffer.len() {
                let row = self.match_buffer[self.match_index].clone();
                self.match_index += 1;
                return Some(row);
            }

            // No more buffered matches. Advance to the next probe row.
            if self.probe_index >= self.orders.len() {
                return None; // exhausted all probe rows
            }

            let order = &self.orders[self.probe_index];
            self.probe_index += 1;

            // Look up the join key in the hash table
            self.match_buffer.clear();
            self.match_index = 0;

            if let Some(matching_customers) = self.hash_table.get(&order.customer_id) {
                for customer in matching_customers {
                    self.match_buffer.push(JoinedRow {
                        order_id: order.id,
                        customer_id: order.customer_id,
                        amount: order.amount,
                        customer_name: customer.name.clone(),
                    });
                }
            }
            // Loop back to check if we got any matches
        }
    }
}
```

The iterator has two states. Either it is draining buffered matches from a probe row that hit multiple build rows, or it is advancing to the next probe row and looking it up. The `loop` handles both transitions cleanly. When the probe side is exhausted, the iterator returns `None`.

### Left Outer Join

An inner join drops probe rows that have no match. A left outer join keeps them, filling in NULLs for the build side. Let's extend the iterator to support both:

```rust,ignore
#[derive(Debug, Clone)]
struct JoinedRowOuter {
    order_id: u32,
    customer_id: u32,
    amount: f64,
    customer_name: Option<String>, // None for unmatched left rows
}

enum JoinType {
    Inner,
    LeftOuter,
}

struct HashJoinIteratorOuter<'a> {
    hash_table: HashMap<u32, Vec<&'a Customer>>,
    orders: &'a [Order],
    probe_index: usize,
    match_buffer: Vec<JoinedRowOuter>,
    match_index: usize,
    join_type: JoinType,
}

impl<'a> HashJoinIteratorOuter<'a> {
    fn new(customers: &'a [Customer], orders: &'a [Order], join_type: JoinType) -> Self {
        let mut hash_table: HashMap<u32, Vec<&'a Customer>> =
            HashMap::with_capacity(customers.len());
        for customer in customers {
            hash_table.entry(customer.id).or_insert_with(Vec::new).push(customer);
        }

        HashJoinIteratorOuter {
            hash_table,
            orders,
            probe_index: 0,
            match_buffer: Vec::new(),
            match_index: 0,
            join_type,
        }
    }
}

impl<'a> Iterator for HashJoinIteratorOuter<'a> {
    type Item = JoinedRowOuter;

    fn next(&mut self) -> Option<JoinedRowOuter> {
        loop {
            if self.match_index < self.match_buffer.len() {
                let row = self.match_buffer[self.match_index].clone();
                self.match_index += 1;
                return Some(row);
            }

            if self.probe_index >= self.orders.len() {
                return None;
            }

            let order = &self.orders[self.probe_index];
            self.probe_index += 1;

            self.match_buffer.clear();
            self.match_index = 0;

            match self.hash_table.get(&order.customer_id) {
                Some(matching_customers) => {
                    for customer in matching_customers {
                        self.match_buffer.push(JoinedRowOuter {
                            order_id: order.id,
                            customer_id: order.customer_id,
                            amount: order.amount,
                            customer_name: Some(customer.name.clone()),
                        });
                    }
                }
                None => {
                    // No match found
                    if matches!(self.join_type, JoinType::LeftOuter) {
                        self.match_buffer.push(JoinedRowOuter {
                            order_id: order.id,
                            customer_id: order.customer_id,
                            amount: order.amount,
                            customer_name: None,
                        });
                    }
                    // For inner join, we skip -- the match_buffer stays empty,
                    // and the loop continues to the next probe row.
                }
            }
        }
    }
}
```

The only difference: when the hash table lookup misses, a left outer join emits a row with `None` for the customer name, while an inner join silently moves on.

### Memory Considerations: What If the Build Side Does Not Fit?

Everything so far assumes the smaller table fits in memory. What if it does not? What if you are joining two tables of 100 million rows each?

The answer is **grace hash join** (also called partitioned hash join). The idea:

1. **Partition both tables** by hashing the join key into N buckets. Rows with the same join key always land in the same bucket partition.
2. **Write each partition to disk** as a temporary file.
3. **Join each partition pair independently** using an in-memory hash join.

Since rows that could match are guaranteed to be in the same partition, joining partitions independently is correct. And each partition is 1/N the size of the original table, so it fits in memory.

```rust,ignore
// Conceptual sketch of grace hash join partitioning
fn partition(rows: &[(u32, String)], num_partitions: usize) -> Vec<Vec<(u32, String)>> {
    let mut partitions = vec![Vec::new(); num_partitions];

    for (key, value) in rows {
        let bucket = (*key as usize) % num_partitions;
        partitions[bucket].push((*key, value.clone()));
    }

    partitions
}

// Then for each partition i:
//   hash_join(left_partitions[i], right_partitions[i])
// Results are the union of all partition joins.
```

This is how PostgreSQL, MySQL, and Spark handle joins that exceed available memory. The partitioning step adds one full read and write of both tables (two passes total), but it makes arbitrarily large joins possible.

---

## The Payoff

Let's run the full hash join against the nested loop join and compare:

```rust
use std::collections::HashMap;

#[derive(Debug, Clone)]
struct Customer {
    id: u32,
    name: String,
}

#[derive(Debug, Clone)]
struct Order {
    id: u32,
    customer_id: u32,
    amount: f64,
}

#[derive(Debug, Clone)]
struct JoinedRow {
    order_id: u32,
    customer_id: u32,
    amount: f64,
    customer_name: String,
}

fn nested_loop_join(orders: &[Order], customers: &[Customer]) -> (Vec<JoinedRow>, u64) {
    let mut results = Vec::new();
    let mut comparisons = 0u64;

    for order in orders {
        for customer in customers {
            comparisons += 1;
            if order.customer_id == customer.id {
                results.push(JoinedRow {
                    order_id: order.id,
                    customer_id: order.customer_id,
                    amount: order.amount,
                    customer_name: customer.name.clone(),
                });
                break;
            }
        }
    }

    (results, comparisons)
}

fn hash_join(orders: &[Order], customers: &[Customer]) -> (Vec<JoinedRow>, u64) {
    let mut comparisons = 0u64;

    // Build phase: hash the smaller table (customers)
    let mut hash_table: HashMap<u32, Vec<&Customer>> =
        HashMap::with_capacity(customers.len());
    for customer in customers {
        comparisons += 1; // count the insert
        hash_table.entry(customer.id).or_insert_with(Vec::new).push(customer);
    }

    // Probe phase: scan the larger table (orders)
    let mut results = Vec::new();
    for order in orders {
        comparisons += 1; // count the lookup
        if let Some(matching) = hash_table.get(&order.customer_id) {
            for customer in matching {
                results.push(JoinedRow {
                    order_id: order.id,
                    customer_id: order.customer_id,
                    amount: order.amount,
                    customer_name: customer.name.clone(),
                });
            }
        }
    }

    (results, comparisons)
}

fn main() {
    let customers: Vec<Customer> = (0..1_000)
        .map(|i| Customer { id: i, name: format!("Customer_{}", i) })
        .collect();

    let orders: Vec<Order> = (0..50_000)
        .map(|i| Order {
            id: i,
            customer_id: i % 1_000,
            amount: 9.99 + i as f64,
        })
        .collect();

    println!("Tables: {} orders x {} customers\n", orders.len(), customers.len());

    let (nl_results, nl_comparisons) = nested_loop_join(&orders, &customers);
    println!("Nested Loop Join:");
    println!("  Results: {} rows", nl_results.len());
    println!("  Comparisons: {}", nl_comparisons);

    let (hj_results, hj_comparisons) = hash_join(&orders, &customers);
    println!("\nHash Join:");
    println!("  Results: {} rows", hj_results.len());
    println!("  Comparisons: {}", hj_comparisons);

    println!("\nSpeedup: {:.0}x fewer operations",
             nl_comparisons as f64 / hj_comparisons as f64);

    // Verify both produce the same results
    assert_eq!(nl_results.len(), hj_results.len());
    println!("\nBoth joins produced {} matching rows. Correct!", nl_results.len());

    // Memory estimate
    let customer_size = std::mem::size_of::<Customer>() + 15; // ~15 bytes avg name
    let hash_table_bytes = customers.len() * (customer_size + 8); // 8 bytes for pointer
    println!("\nHash table memory: ~{} KB", hash_table_bytes / 1024);
    println!("That is the price of {:.0}x faster joins.", nl_comparisons as f64 / hj_comparisons as f64);
}
```

From roughly 25 million comparisons down to about 51,000. The hash join is roughly 490x faster. Scale to 10 million orders and 100,000 customers, and the gap becomes astronomical -- the nested loop join would need 500 billion operations while the hash join needs 10.1 million.

---

## Complexity Table

| Operation | Nested Loop Join | Hash Join | Notes |
|-----------|-----------------|-----------|-------|
| Time | O(n * m) | O(n + m) | n = probe side, m = build side |
| Memory | O(1) extra | O(m) | Hash table for the build side |
| Build phase | N/A | O(m) | One pass to populate HashMap |
| Probe phase | N/A | O(n) | One lookup per probe row |
| Multi-match | O(n * m) still | O(n * k) | k = avg matches per key |
| When build side exceeds RAM | N/A | Grace hash join: O(n + m) with 2 extra disk passes |
| Best case | Still O(n * m) | O(n + m) | Hash join is always fast |
| Works without equality? | Yes (any predicate) | No | Hash join requires equi-join condition |

The key trade-off: hash joins are dramatically faster but require an **equality** join condition (`=`). They cannot handle range conditions like `orders.date > customers.signup_date`. For inequality joins, the nested loop join (or sort-merge join) is still necessary.

---

## Where This Shows Up in Our Database

In Chapter 11, we add JOIN support to our query engine. The query planner chooses between nested loop join and hash join based on table sizes:

```rust,ignore
enum JoinStrategy {
    NestedLoop,
    HashJoin { build_side: TableRef, probe_side: TableRef },
}

fn choose_join_strategy(left_size: usize, right_size: usize) -> JoinStrategy {
    // If either table is very small, nested loop is fine
    if left_size < 100 || right_size < 100 {
        return JoinStrategy::NestedLoop;
    }
    // Otherwise, hash the smaller side
    // ...
}
```

Beyond our toydb, hash joins are ubiquitous:

- **PostgreSQL** uses hash joins as its primary equi-join strategy. The query planner estimates table sizes and chooses hash join when the build side fits in `work_mem`.
- **MySQL 8.0** added hash join support (previously it only had nested loop joins). This was one of the most significant performance improvements in MySQL's history.
- **Apache Spark** uses broadcast hash join (hash the small table, broadcast it to all workers) and shuffle hash join (partition both sides by join key across workers).
- **Pandas** `merge()` function uses hash join internally. When you write `df1.merge(df2, on='id')`, Pandas builds a hash table on one DataFrame and probes with the other.

Any time you see a SQL query with `JOIN ... ON a.id = b.id` finishing in milliseconds instead of hours, a hash join is probably responsible.

---

## Try It Yourself

### Exercise 1: Semi-Join

A semi-join returns rows from the left table that have **at least one** match in the right table, but does not duplicate rows for multiple matches. `SELECT * FROM orders WHERE customer_id IN (SELECT id FROM customers)` is a semi-join. Implement a `HashSemiJoin` iterator that uses the build/probe pattern but returns each probe row at most once.

<details>
<summary>Solution</summary>

```rust
use std::collections::HashSet;

#[derive(Debug, Clone)]
struct Order {
    id: u32,
    customer_id: u32,
    amount: f64,
}

struct HashSemiJoin<'a> {
    // For a semi-join, we only need a HashSet of keys (not full rows)
    build_keys: HashSet<u32>,
    orders: &'a [Order],
    probe_index: usize,
}

impl<'a> HashSemiJoin<'a> {
    fn new(customer_ids: &[u32], orders: &'a [Order]) -> Self {
        let build_keys: HashSet<u32> = customer_ids.iter().copied().collect();
        HashSemiJoin {
            build_keys,
            orders,
            probe_index: 0,
        }
    }
}

impl<'a> Iterator for HashSemiJoin<'a> {
    type Item = &'a Order;

    fn next(&mut self) -> Option<&'a Order> {
        while self.probe_index < self.orders.len() {
            let order = &self.orders[self.probe_index];
            self.probe_index += 1;

            if self.build_keys.contains(&order.customer_id) {
                return Some(order);
            }
        }
        None
    }
}

fn main() {
    let premium_customer_ids: Vec<u32> = (0..100).collect(); // customers 0-99 are "premium"

    let orders: Vec<Order> = (0..10_000)
        .map(|i| Order {
            id: i,
            customer_id: i % 500, // spread across 500 customers
            amount: 9.99 + i as f64,
        })
        .collect();

    let semi_join = HashSemiJoin::new(&premium_customer_ids, &orders);
    let results: Vec<&Order> = semi_join.collect();

    println!("Orders from premium customers: {}", results.len());
    println!("Total orders: {}", orders.len());
    println!("First few matches:");
    for order in results.iter().take(5) {
        println!("  Order {} (customer {}): ${:.2}", order.id, order.customer_id, order.amount);
    }

    // Semi-join uses a HashSet instead of HashMap -- we only need to know
    // if a key exists, not retrieve associated data. This saves memory
    // (no values stored) and is faster (no Vec of matches to iterate).
}
```

</details>

### Exercise 2: Multi-Column Join Key

Real SQL joins often use composite keys: `JOIN ON a.year = b.year AND a.month = b.month`. Modify the hash join to support joining on two columns. You will need a composite key type that implements `Hash` and `Eq`.

<details>
<summary>Solution</summary>

```rust
use std::collections::HashMap;

#[derive(Debug, Clone)]
struct Sale {
    year: u16,
    month: u8,
    region: String,
    revenue: f64,
}

#[derive(Debug, Clone)]
struct Budget {
    year: u16,
    month: u8,
    target: f64,
}

#[derive(Debug)]
struct SaleVsBudget {
    year: u16,
    month: u8,
    region: String,
    revenue: f64,
    target: f64,
}

// Composite key: (year, month)
// Rust tuples automatically implement Hash and Eq if their elements do.
// So (u16, u8) works as a HashMap key out of the box.

fn hash_join_composite(sales: &[Sale], budgets: &[Budget]) -> Vec<SaleVsBudget> {
    // Build phase: hash the smaller table (budgets) on (year, month)
    let mut build_table: HashMap<(u16, u8), Vec<&Budget>> =
        HashMap::with_capacity(budgets.len());

    for budget in budgets {
        build_table
            .entry((budget.year, budget.month))
            .or_insert_with(Vec::new)
            .push(budget);
    }

    // Probe phase: scan sales, look up (year, month)
    let mut results = Vec::new();

    for sale in sales {
        let key = (sale.year, sale.month);
        if let Some(matching_budgets) = build_table.get(&key) {
            for budget in matching_budgets {
                results.push(SaleVsBudget {
                    year: sale.year,
                    month: sale.month,
                    region: sale.region.clone(),
                    revenue: sale.revenue,
                    target: budget.target,
                });
            }
        }
    }

    results
}

fn main() {
    let budgets: Vec<Budget> = (2023..=2024)
        .flat_map(|year| {
            (1..=12).map(move |month| Budget {
                year,
                month,
                target: 100_000.0 + (month as f64 * 5_000.0),
            })
        })
        .collect();

    let regions = ["North", "South", "East", "West"];
    let sales: Vec<Sale> = (2023..=2024u16)
        .flat_map(|year| {
            (1..=12u8).flat_map(move |month| {
                regions.iter().map(move |&region| Sale {
                    year,
                    month,
                    region: region.to_string(),
                    revenue: 80_000.0 + (month as f64 * 3_000.0) + (year as f64 * 100.0),
                })
            })
        })
        .collect();

    println!("Budgets: {} rows", budgets.len());
    println!("Sales: {} rows", sales.len());

    let joined = hash_join_composite(&sales, &budgets);
    println!("Joined: {} rows\n", joined.len());

    println!("Sample results:");
    for row in joined.iter().take(5) {
        let pct = (row.revenue / row.target) * 100.0;
        println!("  {}-{:02} {}: ${:.0} vs ${:.0} target ({:.0}%)",
                 row.year, row.month, row.region, row.revenue, row.target, pct);
    }

    // The key insight: Rust tuples implement Hash and Eq automatically,
    // so (u16, u8) works as a composite HashMap key with no extra code.
    // For more complex composite keys, you would derive Hash and Eq
    // on a custom struct.
}
```

</details>

### Exercise 3: Join with Aggregation

Build a hash join that computes aggregate statistics during the probe phase instead of materializing all joined rows. For each customer, compute the total order amount and order count. This simulates `SELECT customer_name, COUNT(*), SUM(amount) FROM orders JOIN customers ON ... GROUP BY customer_name`.

<details>
<summary>Solution</summary>

```rust
use std::collections::HashMap;

#[derive(Debug, Clone)]
struct Customer {
    id: u32,
    name: String,
}

#[derive(Debug, Clone)]
struct Order {
    id: u32,
    customer_id: u32,
    amount: f64,
}

#[derive(Debug)]
struct CustomerSummary {
    name: String,
    order_count: u32,
    total_amount: f64,
}

fn join_and_aggregate(
    customers: &[Customer],
    orders: &[Order],
) -> Vec<CustomerSummary> {
    // Build phase: hash customers by ID
    let mut customer_lookup: HashMap<u32, &Customer> =
        HashMap::with_capacity(customers.len());
    for customer in customers {
        customer_lookup.insert(customer.id, customer);
    }

    // Probe phase with inline aggregation
    // Instead of materializing joined rows, we accumulate directly
    let mut aggregates: HashMap<u32, (String, u32, f64)> = HashMap::new();

    for order in orders {
        if let Some(customer) = customer_lookup.get(&order.customer_id) {
            let entry = aggregates
                .entry(customer.id)
                .or_insert_with(|| (customer.name.clone(), 0, 0.0));
            entry.1 += 1;          // increment count
            entry.2 += order.amount; // sum amount
        }
    }

    // Convert to result structs, sorted by total amount descending
    let mut results: Vec<CustomerSummary> = aggregates
        .into_values()
        .map(|(name, count, total)| CustomerSummary {
            name,
            order_count: count,
            total_amount: total,
        })
        .collect();

    results.sort_by(|a, b| b.total_amount.partial_cmp(&a.total_amount).unwrap());
    results
}

fn main() {
    let customers: Vec<Customer> = (0..100)
        .map(|i| Customer { id: i, name: format!("Customer_{}", i) })
        .collect();

    // Each customer gets a random-ish number of orders
    let orders: Vec<Order> = (0..10_000)
        .map(|i| Order {
            id: i,
            customer_id: (i * 7 + 13) % 100, // pseudo-random distribution
            amount: 10.0 + (i % 50) as f64 * 2.5,
        })
        .collect();

    let summaries = join_and_aggregate(&customers, &orders);

    println!("Top 10 customers by revenue:\n");
    println!("{:<15} {:>8} {:>12}", "Customer", "Orders", "Total");
    println!("{}", "-".repeat(37));

    for summary in summaries.iter().take(10) {
        println!("{:<15} {:>8} {:>12.2}",
                 summary.name, summary.order_count, summary.total_amount);
    }

    println!("\n{} customers with orders", summaries.len());
    let total_revenue: f64 = summaries.iter().map(|s| s.total_amount).sum();
    println!("Total revenue: ${:.2}", total_revenue);

    // By aggregating during the probe phase, we avoid materializing
    // 10,000 joined rows. Memory usage is proportional to the number
    // of groups (100 customers), not the number of matches (10,000).
    // This is how real query engines implement "hash aggregate" --
    // the aggregation runs inside the join operator.
}
```

</details>

---

## Recap

A hash join turns an O(n * m) nested loop into an O(n + m) two-pass algorithm. Build a hash table on the smaller table, probe it with the larger table. Each probe is an O(1) hash lookup instead of an O(m) linear scan. The memory cost is the hash table for the build side -- which is why you always hash the smaller table.

When the build side does not fit in memory, grace hash join partitions both tables by hash value and joins each partition pair independently. This adds two disk passes but handles arbitrarily large inputs.

The hash join has one limitation: it only works for equi-joins (equality conditions on the join key). For range joins or inequality predicates, you need different algorithms -- sort-merge join or nested loop with an index. But for the vast majority of SQL joins, which are equi-joins on foreign keys, the hash join is the workhorse that makes interactive query performance possible.
