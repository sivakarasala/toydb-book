# B-Tree — "The filing cabinet that sorts itself"

You just built a hash table and it is blazing fast for exact lookups. `GET user:42` -- done in O(1). But now a client sends this: "Give me all customers with IDs between 1000 and 2000." You stare at your hash table and realize it cannot help. Hash functions scatter keys randomly across buckets. Keys 1000, 1001, and 1002 might live in buckets 7, 4391, and 82. To find a range, you would have to scan every bucket -- back to O(n).

You need a data structure that keeps keys **sorted** so that range queries are natural. You need a B-tree.

---

## The Problem

Imagine a filing cabinet in a records office. You have 100,000 customer records. A manager asks: "Pull all the records for customers 1000 through 2000." If the files are in random order (like a hash table), you open every drawer and check every folder. That is 100,000 checks.

But if the filing cabinet is sorted by customer ID, you walk straight to the drawer labeled "900-1100", flip to 1000, and pull folders sequentially until you hit 2000. Maybe you open 3 drawers. Maybe you check 1,001 folders. But you never touch the other 99,000.

That is what a B-tree gives you. It is a sorted, balanced tree where each node holds multiple keys, and you can walk to any key in O(log n) steps. Range queries fall out naturally -- once you find the start, you walk forward.

Let's quantify the cost of not having one:

```rust
fn main() {
    // 100,000 customer IDs in random order (simulating hash table layout)
    let mut ids: Vec<u32> = (0..100_000).collect();
    // Shuffle to simulate hash-scattered ordering
    // (In a real hash table, the order depends on hash values)

    // Range query: find all IDs between 1000 and 2000
    let mut results = Vec::new();
    let mut comparisons = 0u64;
    for &id in &ids {
        comparisons += 1;
        if id >= 1000 && id <= 2000 {
            results.push(id);
        }
    }

    println!("Found {} records in range [1000, 2000]", results.len());
    println!("Required {} comparisons (full scan)", comparisons);
    // Found 1001 records in range [1000, 2000]
    // Required 100000 comparisons (full scan)
}
```

100,000 comparisons to find 1,001 records. With a B-tree, we would need about 17 comparisons to find the start, then 1,001 sequential reads. That is a 98% reduction.

---

## The Insight

A binary search tree keeps things sorted, but it can become unbalanced -- a sorted insertion sequence turns it into a linked list, and lookups degrade to O(n). A B-tree solves this with two ideas:

1. **Each node holds multiple keys.** Instead of one key per node (like a binary tree), a B-tree node holds up to `2t - 1` keys, where `t` is the **minimum degree** (typically 2 or more). More keys per node means fewer levels, which means fewer disk reads.

2. **The tree stays perfectly balanced.** All leaves live at the same depth, always. When a node overflows, it splits into two and pushes the middle key up to the parent. The tree grows from the root upward, not from the leaves downward.

Think of it as a filing cabinet where each drawer holds multiple folders (not just one), and when a drawer gets too full, the office buys a new cabinet and reorganizes the drawers to keep everything at the same height. The manager can always find any folder by opening the same number of drawers -- guaranteed.

For a B-tree with minimum degree `t = 3`:
- Each internal node holds 2 to 5 keys
- Each internal node has 3 to 6 children
- With 100,000 entries, the tree is about 7 levels deep
- Any lookup takes at most 7 steps

Let's build one.

---

## The Build

We will build a simplified B-tree that supports `search` and `insert`. We will use `t = 3` (minimum degree 3), meaning each node holds between 2 and 5 keys.

### The Node

A B-tree node is either a leaf (holds keys and values, no children) or an internal node (holds keys, values, and child pointers). We will unify them into one struct with a flag:

```rust,ignore
struct BTreeNode<K: Ord + Clone + std::fmt::Debug, V: Clone + std::fmt::Debug> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<BTreeNode<K, V>>,
    leaf: bool,
}
```

In a real database's B-tree, each node is a disk page (typically 4KB or 16KB). Children are not pointers in memory -- they are page numbers on disk. But for understanding the algorithm, in-memory nodes work perfectly.

### The Full Implementation

Here is the complete B-tree. It is about 150 lines, but every line earns its place:

```rust
use std::fmt;

const T: usize = 3; // minimum degree: each node has 2..5 keys, 3..6 children

#[derive(Clone)]
struct BTreeNode<K: Ord + Clone + fmt::Debug, V: Clone + fmt::Debug> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<BTreeNode<K, V>>,
    leaf: bool,
}

impl<K: Ord + Clone + fmt::Debug, V: Clone + fmt::Debug> BTreeNode<K, V> {
    fn new_leaf() -> Self {
        BTreeNode {
            keys: Vec::new(),
            values: Vec::new(),
            children: Vec::new(),
            leaf: true,
        }
    }

    fn new_internal() -> Self {
        BTreeNode {
            keys: Vec::new(),
            values: Vec::new(),
            children: Vec::new(),
            leaf: false,
        }
    }

    fn is_full(&self) -> bool {
        self.keys.len() == 2 * T - 1
    }
}

struct BTree<K: Ord + Clone + fmt::Debug, V: Clone + fmt::Debug> {
    root: BTreeNode<K, V>,
}

impl<K: Ord + Clone + fmt::Debug, V: Clone + fmt::Debug> BTree<K, V> {
    fn new() -> Self {
        BTree {
            root: BTreeNode::new_leaf(),
        }
    }

    /// Search for a key, returning a reference to its value if found.
    fn search(&self, key: &K) -> Option<&V> {
        Self::search_node(&self.root, key)
    }

    fn search_node<'a>(node: &'a BTreeNode<K, V>, key: &K) -> Option<&'a V> {
        // Find the first key >= our target
        let mut i = 0;
        while i < node.keys.len() && *key > node.keys[i] {
            i += 1;
        }

        // Did we find an exact match?
        if i < node.keys.len() && *key == node.keys[i] {
            return Some(&node.values[i]);
        }

        // If this is a leaf, the key is not in the tree
        if node.leaf {
            return None;
        }

        // Recurse into the appropriate child
        Self::search_node(&node.children[i], key)
    }

    /// Insert a key-value pair. If the key exists, update its value.
    fn insert(&mut self, key: K, value: V) {
        // If root is full, split it first -- this is the only way the tree grows taller
        if self.root.is_full() {
            let old_root = std::mem::replace(&mut self.root, BTreeNode::new_internal());
            self.root.children.push(old_root);
            Self::split_child(&mut self.root, 0);
        }
        Self::insert_non_full(&mut self.root, key, value);
    }

    /// Split the i-th child of `parent`, which must be full (2t-1 keys).
    /// After splitting:
    /// - The child keeps its first t-1 keys
    /// - A new sibling gets the last t-1 keys
    /// - The median key moves up into the parent
    fn split_child(parent: &mut BTreeNode<K, V>, i: usize) {
        let child = &mut parent.children[i];
        let mid = T - 1; // index of the median key

        // Create the new right sibling with the upper half
        let mut sibling = if child.leaf {
            BTreeNode::new_leaf()
        } else {
            BTreeNode::new_internal()
        };

        // Move keys[mid+1..] and values[mid+1..] to the sibling
        sibling.keys = child.keys.split_off(mid + 1);
        sibling.values = child.values.split_off(mid + 1);

        // If not a leaf, move the upper children too
        if !child.leaf {
            sibling.children = child.children.split_off(mid + 1);
        }

        // Pop the median key/value from the child -- it will move up
        let median_key = child.keys.pop().unwrap();
        let median_value = child.values.pop().unwrap();

        // Insert median into parent, and sibling as a new child
        parent.keys.insert(i, median_key);
        parent.values.insert(i, median_value);
        parent.children.insert(i + 1, sibling);
    }

    /// Insert into a node that is guaranteed to be non-full.
    fn insert_non_full(node: &mut BTreeNode<K, V>, key: K, value: V) {
        let mut i = node.keys.len();

        if node.leaf {
            // Find the correct position and insert
            while i > 0 && key < node.keys[i - 1] {
                i -= 1;
            }
            // Check for duplicate key
            if i < node.keys.len() && key == node.keys[i] {
                node.values[i] = value; // update existing
                return;
            }
            node.keys.insert(i, key);
            node.values.insert(i, value);
        } else {
            // Find the child to descend into
            while i > 0 && key < node.keys[i - 1] {
                i -= 1;
            }
            // Check for duplicate at this internal node
            if i < node.keys.len() && key == node.keys[i] {
                node.values[i] = value;
                return;
            }
            // If the target child is full, split it first
            if node.children[i].is_full() {
                Self::split_child(node, i);
                // After split, the median moved up. Decide which side to go to.
                if key > node.keys[i] {
                    i += 1;
                } else if key == node.keys[i] {
                    node.values[i] = value;
                    return;
                }
            }
            Self::insert_non_full(&mut node.children[i], key, value);
        }
    }

    /// Range query: find all key-value pairs where lo <= key <= hi.
    fn range(&self, lo: &K, hi: &K) -> Vec<(&K, &V)> {
        let mut results = Vec::new();
        Self::range_collect(&self.root, lo, hi, &mut results);
        results
    }

    fn range_collect<'a>(
        node: &'a BTreeNode<K, V>,
        lo: &K,
        hi: &K,
        results: &mut Vec<(&'a K, &'a V)>,
    ) {
        let mut i = 0;

        // Skip keys smaller than lo
        while i < node.keys.len() && node.keys[i] < *lo {
            i += 1;
        }

        // Collect keys in range
        while i < node.keys.len() && node.keys[i] <= *hi {
            // If not a leaf, collect from the left child first (in-order)
            if !node.leaf {
                Self::range_collect(&node.children[i], lo, hi, results);
            }
            results.push((&node.keys[i], &node.values[i]));
            i += 1;
        }

        // If not a leaf, collect from the rightmost relevant child
        if !node.leaf && i < node.children.len() {
            Self::range_collect(&node.children[i], lo, hi, results);
        }
    }

    /// Return the height of the tree (all leaves should be at the same depth).
    fn height(&self) -> usize {
        let mut h = 0;
        let mut node = &self.root;
        while !node.leaf {
            h += 1;
            node = &node.children[0];
        }
        h
    }

    /// Count total keys stored.
    fn len(&self) -> usize {
        Self::count_keys(&self.root)
    }

    fn count_keys(node: &BTreeNode<K, V>) -> usize {
        let mut count = node.keys.len();
        for child in &node.children {
            count += Self::count_keys(child);
        }
        count
    }
}

fn main() {
    let mut tree = BTree::new();

    // Insert 100,000 customer records
    for i in 0u32..100_000 {
        tree.insert(i, format!("Customer #{}", i));
    }

    println!("B-tree with {} entries", tree.len());
    println!("Height: {} levels", tree.height());
    println!("Min degree t={}, so each node has {}-{} keys", T, T - 1, 2 * T - 1);

    // Exact lookup
    if let Some(val) = tree.search(&42) {
        println!("\nExact lookup: key 42 -> {}", val);
    }

    // The killer feature: range queries
    let results = tree.range(&1000, &2000);
    println!("\nRange query [1000, 2000]: found {} records", results.len());
    println!("First 5:");
    for (k, v) in results.iter().take(5) {
        println!("  {} -> {}", k, v);
    }
    println!("  ...");
    println!("Last 5:");
    for (k, v) in results.iter().rev().take(5).collect::<Vec<_>>().iter().rev() {
        println!("  {} -> {}", k, v);
    }

    // Compare with hash table approach
    println!("\n--- Comparison ---");
    println!("Hash table range query: O(n) = 100,000 comparisons");
    println!("B-tree range query:     O(log n + k) = ~{} + 1,001 = ~{} operations",
             tree.height(), tree.height() + 1001);
    println!("Speedup: ~{}x", 100_000 / (tree.height() + 1001));
}
```

Let's walk through the key ideas.

### Search: Walking Down the Tree

Searching a B-tree is like navigating a filing cabinet. At each node, you scan through the keys to find where your target belongs. If you find an exact match, great -- return the value. If not, you follow the child pointer between the two keys that bracket your target.

At each level, you do at most `2t - 1` comparisons (the maximum number of keys in a node). With `t = 3`, that is at most 5 comparisons per level. For 100,000 entries, the tree is about 7 levels deep. So a lookup takes at most 35 comparisons. Compare that with 50,000 average comparisons for a linear scan.

### Insert: The Bottom-Up Split

Insertion is where the B-tree earns its keep. The algorithm has two rules:

1. **Always insert into a leaf.** Walk down the tree to find the right leaf, then insert the key in sorted order.
2. **If a node is full before you enter it, split it first.** This guarantees that you always have room.

The split operation is the heart of B-tree balance. When a node has `2t - 1` keys (the maximum), you cut it in half:
- The lower `t - 1` keys stay in the original node
- The upper `t - 1` keys move to a new sibling
- The middle key moves **up** into the parent

This is why all leaves stay at the same depth. The tree only grows taller when the root splits, and when the root splits, every path from root to leaf gets one level longer simultaneously.

### Range Queries: Walk and Collect

Here is the trick that hash tables cannot do. Once you find the first key in your range, you traverse the tree in-order, collecting every key until you pass the upper bound. The B-tree's sorted structure means the next key is either the next slot in the current node or the leftmost key in the next child. No random jumping, no scanning irrelevant data.

The cost: O(log n) to find the start, plus O(k) to collect k results. For our range query of 1,001 results from a tree of height 7, that is 7 + 1,001 = 1,008 operations. The linear scan needed 100,000.

---

## The Payoff

The B-tree gives us something the hash table fundamentally cannot: **order**. With order comes:

- **Range queries**: all keys between A and B
- **Sorted iteration**: scan all keys in order
- **Min/max**: the leftmost and rightmost leaves
- **Prefix queries**: all keys starting with "user:abc"

The hash table wins on exact lookups (O(1) vs O(log n)), but the B-tree wins everywhere else. Most databases use both: hash indexes for equality queries, B-tree indexes for everything else.

---

## Complexity Table

| Operation | Linear Scan | Hash Table | B-Tree |
|-----------|------------|------------|--------|
| Exact lookup | O(n) | O(1) amortized | O(log n) |
| Insert | O(1) append | O(1) amortized | O(log n) |
| Delete | O(n) | O(1) amortized | O(log n) |
| Range query (k results) | O(n) | O(n) | O(log n + k) |
| Min / Max | O(n) | O(n) | O(log n) |
| Sorted iteration | O(n log n) sort first | O(n log n) sort first | O(n) |
| Space | O(n) | O(n) | O(n) |

The O(log n) for B-tree operations is a very small number in practice. With minimum degree t = 100 (typical for disk-based B-trees), a tree of height 3 can hold 100 million entries. Three disk reads for any lookup in a hundred-million-row table.

---

## Where This Shows Up in Our Database

In Chapter 2, we build `MemoryStorage` using Rust's standard `BTreeMap`:

```rust,ignore
use std::collections::BTreeMap;

pub struct MemoryStorage {
    data: BTreeMap<String, Vec<u8>>,
}

impl MemoryStorage {
    pub fn scan(&self, start: &str, end: &str) -> Vec<(&String, &Vec<u8>)> {
        self.data.range(start.to_string()..=end.to_string()).collect()
    }
}
```

That `.range()` call does exactly what our `range()` method does -- walks the B-tree to the start key, then collects entries until the end key. Rust's `BTreeMap` uses the same algorithm we just built, with a minimum degree optimized for cache line size rather than disk page size.

In real database systems, B-trees are everywhere:
- **PostgreSQL and MySQL** use B-tree indexes as their default index type
- **SQLite** stores entire tables as B-trees (the table *is* the index)
- **InnoDB** (MySQL's storage engine) uses a clustered B-tree where the primary key index contains the actual row data
- **MongoDB** uses B-trees for its default `_id` index

The B-tree is arguably the most important data structure in all of database engineering. It has been the backbone of relational databases since the 1970s, and nothing has replaced it for range-query workloads.

---

## Try It Yourself

### Exercise 1: In-Order Traversal

Implement a method `fn in_order(&self) -> Vec<(&K, &V)>` that returns all key-value pairs in sorted order. Use it to verify that the B-tree maintains its sort invariant after inserting 10,000 keys in random order. (Hint: insert keys in order 7, 3, 9, 1, 5, ... and check that in_order returns them as 1, 3, 5, 7, 9, ...)

<details>
<summary>Solution</summary>

```rust
use std::fmt;

const T: usize = 3;

#[derive(Clone)]
struct BTreeNode<K: Ord + Clone + fmt::Debug, V: Clone + fmt::Debug> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<BTreeNode<K, V>>,
    leaf: bool,
}

impl<K: Ord + Clone + fmt::Debug, V: Clone + fmt::Debug> BTreeNode<K, V> {
    fn new_leaf() -> Self {
        BTreeNode { keys: Vec::new(), values: Vec::new(), children: Vec::new(), leaf: true }
    }
    fn new_internal() -> Self {
        BTreeNode { keys: Vec::new(), values: Vec::new(), children: Vec::new(), leaf: false }
    }
    fn is_full(&self) -> bool {
        self.keys.len() == 2 * T - 1
    }
}

struct BTree<K: Ord + Clone + fmt::Debug, V: Clone + fmt::Debug> {
    root: BTreeNode<K, V>,
}

impl<K: Ord + Clone + fmt::Debug, V: Clone + fmt::Debug> BTree<K, V> {
    fn new() -> Self {
        BTree { root: BTreeNode::new_leaf() }
    }

    fn insert(&mut self, key: K, value: V) {
        if self.root.is_full() {
            let old_root = std::mem::replace(&mut self.root, BTreeNode::new_internal());
            self.root.children.push(old_root);
            Self::split_child(&mut self.root, 0);
        }
        Self::insert_non_full(&mut self.root, key, value);
    }

    fn split_child(parent: &mut BTreeNode<K, V>, i: usize) {
        let child = &mut parent.children[i];
        let mid = T - 1;
        let mut sibling = if child.leaf { BTreeNode::new_leaf() } else { BTreeNode::new_internal() };
        sibling.keys = child.keys.split_off(mid + 1);
        sibling.values = child.values.split_off(mid + 1);
        if !child.leaf {
            sibling.children = child.children.split_off(mid + 1);
        }
        let median_key = child.keys.pop().unwrap();
        let median_value = child.values.pop().unwrap();
        parent.keys.insert(i, median_key);
        parent.values.insert(i, median_value);
        parent.children.insert(i + 1, sibling);
    }

    fn insert_non_full(node: &mut BTreeNode<K, V>, key: K, value: V) {
        let mut i = node.keys.len();
        if node.leaf {
            while i > 0 && key < node.keys[i - 1] { i -= 1; }
            if i < node.keys.len() && key == node.keys[i] {
                node.values[i] = value;
                return;
            }
            node.keys.insert(i, key);
            node.values.insert(i, value);
        } else {
            while i > 0 && key < node.keys[i - 1] { i -= 1; }
            if i < node.keys.len() && key == node.keys[i] {
                node.values[i] = value;
                return;
            }
            if node.children[i].is_full() {
                Self::split_child(node, i);
                if key > node.keys[i] {
                    i += 1;
                } else if key == node.keys[i] {
                    node.values[i] = value;
                    return;
                }
            }
            Self::insert_non_full(&mut node.children[i], key, value);
        }
    }

    fn in_order(&self) -> Vec<(&K, &V)> {
        let mut results = Vec::new();
        Self::in_order_collect(&self.root, &mut results);
        results
    }

    fn in_order_collect<'a>(
        node: &'a BTreeNode<K, V>,
        results: &mut Vec<(&'a K, &'a V)>,
    ) {
        for i in 0..node.keys.len() {
            // Visit left child before each key
            if !node.leaf {
                Self::in_order_collect(&node.children[i], results);
            }
            results.push((&node.keys[i], &node.values[i]));
        }
        // Visit the rightmost child
        if !node.leaf && !node.children.is_empty() {
            Self::in_order_collect(node.children.last().unwrap(), results);
        }
    }
}

fn main() {
    let mut tree = BTree::new();

    // Insert in scrambled order
    let keys = vec![50, 25, 75, 12, 37, 62, 87, 6, 18, 31, 43, 56, 68, 81, 93];
    for &k in &keys {
        tree.insert(k, format!("val-{}", k));
    }

    let sorted = tree.in_order();
    println!("In-order traversal:");
    for (k, v) in &sorted {
        println!("  {} -> {}", k, v);
    }

    // Verify sorted order
    let is_sorted = sorted.windows(2).all(|w| w[0].0 <= w[1].0);
    println!("\nIs sorted: {}", is_sorted);

    // Larger test: insert 10,000 keys in a pseudo-random order
    let mut tree2 = BTree::new();
    let mut rng_state: u64 = 12345;
    let mut random_keys: Vec<u32> = (0..10_000).collect();
    // Simple Fisher-Yates shuffle with LCG
    for i in (1..random_keys.len()).rev() {
        rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let j = (rng_state >> 33) as usize % (i + 1);
        random_keys.swap(i, j);
    }

    for &k in &random_keys {
        tree2.insert(k, k);
    }

    let sorted2 = tree2.in_order();
    let is_sorted2 = sorted2.windows(2).all(|w| w[0].0 <= w[1].0);
    println!("10,000 random insertions, in-order is sorted: {}", is_sorted2);
    println!("First 5: {:?}", sorted2.iter().take(5).map(|(k, _)| *k).collect::<Vec<_>>());
    println!("Last 5: {:?}", sorted2.iter().rev().take(5).map(|(k, _)| *k).collect::<Vec<_>>());
}
```

</details>

### Exercise 2: Node Statistics

Add a method `fn stats(&self) -> (usize, usize, usize, usize)` that returns `(total_nodes, leaf_nodes, internal_nodes, height)`. Insert 1,000 entries and print the stats. How does the number of nodes relate to the minimum degree `t`?

<details>
<summary>Solution</summary>

```rust
use std::fmt;

const T: usize = 3;

#[derive(Clone)]
struct BTreeNode<K: Ord + Clone + fmt::Debug, V: Clone + fmt::Debug> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<BTreeNode<K, V>>,
    leaf: bool,
}

impl<K: Ord + Clone + fmt::Debug, V: Clone + fmt::Debug> BTreeNode<K, V> {
    fn new_leaf() -> Self {
        BTreeNode { keys: Vec::new(), values: Vec::new(), children: Vec::new(), leaf: true }
    }
    fn new_internal() -> Self {
        BTreeNode { keys: Vec::new(), values: Vec::new(), children: Vec::new(), leaf: false }
    }
    fn is_full(&self) -> bool {
        self.keys.len() == 2 * T - 1
    }
}

struct BTree<K: Ord + Clone + fmt::Debug, V: Clone + fmt::Debug> {
    root: BTreeNode<K, V>,
}

impl<K: Ord + Clone + fmt::Debug, V: Clone + fmt::Debug> BTree<K, V> {
    fn new() -> Self {
        BTree { root: BTreeNode::new_leaf() }
    }

    fn insert(&mut self, key: K, value: V) {
        if self.root.is_full() {
            let old_root = std::mem::replace(&mut self.root, BTreeNode::new_internal());
            self.root.children.push(old_root);
            Self::split_child(&mut self.root, 0);
        }
        Self::insert_non_full(&mut self.root, key, value);
    }

    fn split_child(parent: &mut BTreeNode<K, V>, i: usize) {
        let child = &mut parent.children[i];
        let mid = T - 1;
        let mut sibling = if child.leaf { BTreeNode::new_leaf() } else { BTreeNode::new_internal() };
        sibling.keys = child.keys.split_off(mid + 1);
        sibling.values = child.values.split_off(mid + 1);
        if !child.leaf { sibling.children = child.children.split_off(mid + 1); }
        let mk = child.keys.pop().unwrap();
        let mv = child.values.pop().unwrap();
        parent.keys.insert(i, mk);
        parent.values.insert(i, mv);
        parent.children.insert(i + 1, sibling);
    }

    fn insert_non_full(node: &mut BTreeNode<K, V>, key: K, value: V) {
        let mut i = node.keys.len();
        if node.leaf {
            while i > 0 && key < node.keys[i - 1] { i -= 1; }
            if i < node.keys.len() && key == node.keys[i] { node.values[i] = value; return; }
            node.keys.insert(i, key);
            node.values.insert(i, value);
        } else {
            while i > 0 && key < node.keys[i - 1] { i -= 1; }
            if i < node.keys.len() && key == node.keys[i] { node.values[i] = value; return; }
            if node.children[i].is_full() {
                Self::split_child(node, i);
                if key > node.keys[i] { i += 1; }
                else if key == node.keys[i] { node.values[i] = value; return; }
            }
            Self::insert_non_full(&mut node.children[i], key, value);
        }
    }

    fn stats(&self) -> (usize, usize, usize, usize) {
        let mut total = 0;
        let mut leaves = 0;
        let mut internals = 0;
        Self::count_nodes(&self.root, &mut total, &mut leaves, &mut internals);
        let height = self.height();
        (total, leaves, internals, height)
    }

    fn count_nodes(
        node: &BTreeNode<K, V>,
        total: &mut usize,
        leaves: &mut usize,
        internals: &mut usize,
    ) {
        *total += 1;
        if node.leaf {
            *leaves += 1;
        } else {
            *internals += 1;
            for child in &node.children {
                Self::count_nodes(child, total, leaves, internals);
            }
        }
    }

    fn height(&self) -> usize {
        let mut h = 0;
        let mut node = &self.root;
        while !node.leaf {
            h += 1;
            node = &node.children[0];
        }
        h
    }

    fn len(&self) -> usize {
        Self::count_keys(&self.root)
    }

    fn count_keys(node: &BTreeNode<K, V>) -> usize {
        let mut count = node.keys.len();
        for child in &node.children { count += Self::count_keys(child); }
        count
    }
}

fn main() {
    let mut tree = BTree::new();
    for i in 0u32..1_000 {
        tree.insert(i, i);
    }

    let (total, leaves, internals, height) = tree.stats();
    println!("B-Tree stats for 1,000 entries (t={}):", T);
    println!("  Total keys:      {}", tree.len());
    println!("  Total nodes:     {}", total);
    println!("  Leaf nodes:      {}", leaves);
    println!("  Internal nodes:  {}", internals);
    println!("  Height:          {}", height);
    println!("  Avg keys/leaf:   {:.1}", tree.len() as f64 / leaves as f64);
    println!();
    println!("With t={}, each node holds {}-{} keys.", T, T - 1, 2 * T - 1);
    println!("A perfectly packed tree of height {} can hold up to {} keys.",
             height, (2 * T).pow(height as u32 + 1) - 1);
    // With larger t, each node holds more keys, so you need
    // fewer nodes and fewer levels. Disk-based B-trees use
    // t=100+ to minimize the number of page reads.
}
```

</details>

### Exercise 3: Delete (Challenge)

Implement `fn delete(&mut self, key: &K) -> Option<V>`. B-tree deletion has three cases:
1. Key is in a leaf -- just remove it (but handle underflow if the leaf drops below t-1 keys)
2. Key is in an internal node -- replace it with its in-order predecessor or successor, then delete that
3. Before descending into a child with exactly t-1 keys, ensure it has at least t keys (by rotating from a sibling or merging)

This is the hardest B-tree operation. Start with case 1 only (delete from a non-underflowing leaf), then tackle the full algorithm.

<details>
<summary>Solution (simplified: leaf deletion with underflow handling)</summary>

```rust
use std::fmt;

const T: usize = 3;

#[derive(Clone)]
struct BTreeNode<K: Ord + Clone + fmt::Debug, V: Clone + fmt::Debug> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<BTreeNode<K, V>>,
    leaf: bool,
}

impl<K: Ord + Clone + fmt::Debug, V: Clone + fmt::Debug> BTreeNode<K, V> {
    fn new_leaf() -> Self {
        BTreeNode { keys: Vec::new(), values: Vec::new(), children: Vec::new(), leaf: true }
    }
    fn new_internal() -> Self {
        BTreeNode { keys: Vec::new(), values: Vec::new(), children: Vec::new(), leaf: false }
    }
    fn is_full(&self) -> bool { self.keys.len() == 2 * T - 1 }
    fn is_minimum(&self) -> bool { self.keys.len() == T - 1 }
}

struct BTree<K: Ord + Clone + fmt::Debug, V: Clone + fmt::Debug> {
    root: BTreeNode<K, V>,
}

impl<K: Ord + Clone + fmt::Debug, V: Clone + fmt::Debug> BTree<K, V> {
    fn new() -> Self { BTree { root: BTreeNode::new_leaf() } }

    fn insert(&mut self, key: K, value: V) {
        if self.root.is_full() {
            let old_root = std::mem::replace(&mut self.root, BTreeNode::new_internal());
            self.root.children.push(old_root);
            Self::split_child(&mut self.root, 0);
        }
        Self::insert_non_full(&mut self.root, key, value);
    }

    fn split_child(parent: &mut BTreeNode<K, V>, i: usize) {
        let child = &mut parent.children[i];
        let mid = T - 1;
        let mut sibling = if child.leaf { BTreeNode::new_leaf() } else { BTreeNode::new_internal() };
        sibling.keys = child.keys.split_off(mid + 1);
        sibling.values = child.values.split_off(mid + 1);
        if !child.leaf { sibling.children = child.children.split_off(mid + 1); }
        let mk = child.keys.pop().unwrap();
        let mv = child.values.pop().unwrap();
        parent.keys.insert(i, mk);
        parent.values.insert(i, mv);
        parent.children.insert(i + 1, sibling);
    }

    fn insert_non_full(node: &mut BTreeNode<K, V>, key: K, value: V) {
        let mut i = node.keys.len();
        if node.leaf {
            while i > 0 && key < node.keys[i - 1] { i -= 1; }
            if i < node.keys.len() && key == node.keys[i] { node.values[i] = value; return; }
            node.keys.insert(i, key);
            node.values.insert(i, value);
        } else {
            while i > 0 && key < node.keys[i - 1] { i -= 1; }
            if i < node.keys.len() && key == node.keys[i] { node.values[i] = value; return; }
            if node.children[i].is_full() {
                Self::split_child(node, i);
                if key > node.keys[i] { i += 1; }
                else if key == node.keys[i] { node.values[i] = value; return; }
            }
            Self::insert_non_full(&mut node.children[i], key, value);
        }
    }

    fn search(&self, key: &K) -> Option<&V> {
        Self::search_node(&self.root, key)
    }

    fn search_node<'a>(node: &'a BTreeNode<K, V>, key: &K) -> Option<&'a V> {
        let mut i = 0;
        while i < node.keys.len() && *key > node.keys[i] { i += 1; }
        if i < node.keys.len() && *key == node.keys[i] { return Some(&node.values[i]); }
        if node.leaf { return None; }
        Self::search_node(&node.children[i], key)
    }

    /// Delete a key from the B-tree.
    fn delete(&mut self, key: &K) -> Option<V> {
        let result = Self::delete_from_node(&mut self.root, key);

        // If root has no keys but has a child, shrink the tree
        if self.root.keys.is_empty() && !self.root.leaf {
            self.root = self.root.children.remove(0);
        }

        result
    }

    fn delete_from_node(node: &mut BTreeNode<K, V>, key: &K) -> Option<V> {
        let mut i = 0;
        while i < node.keys.len() && *key > node.keys[i] { i += 1; }

        // Case 1: Key found in this node
        if i < node.keys.len() && *key == node.keys[i] {
            if node.leaf {
                // Case 1a: Key is in a leaf -- just remove it
                node.keys.remove(i);
                return Some(node.values.remove(i));
            }

            // Case 1b: Key is in an internal node
            // Replace with in-order predecessor (max key in left subtree)
            if node.children[i].keys.len() >= T {
                let (pred_key, pred_val) = Self::remove_max(&mut node.children[i]);
                node.keys[i] = pred_key;
                let old_val = std::mem::replace(&mut node.values[i], pred_val);
                return Some(old_val);
            }
            // Or in-order successor (min key in right subtree)
            if node.children[i + 1].keys.len() >= T {
                let (succ_key, succ_val) = Self::remove_min(&mut node.children[i + 1]);
                node.keys[i] = succ_key;
                let old_val = std::mem::replace(&mut node.values[i], succ_val);
                return Some(old_val);
            }
            // Both children have t-1 keys: merge them
            Self::merge_children(node, i);
            return Self::delete_from_node(&mut node.children[i], key);
        }

        // Case 2: Key not found in this node
        if node.leaf {
            return None; // Key does not exist
        }

        // Ensure child[i] has at least t keys before descending
        if node.children[i].keys.len() < T {
            Self::fill_child(node, i);
            // After filling, the key might have moved; re-find position
            i = 0;
            while i < node.keys.len() && *key > node.keys[i] { i += 1; }
            if i < node.keys.len() && *key == node.keys[i] {
                // Key got merged into this node; handle it here
                return Self::delete_from_node(node, key);
            }
        }

        Self::delete_from_node(&mut node.children[i], key)
    }

    fn remove_max(node: &mut BTreeNode<K, V>) -> (K, V) {
        if node.leaf {
            let k = node.keys.pop().unwrap();
            let v = node.values.pop().unwrap();
            return (k, v);
        }
        let last = node.children.len() - 1;
        if node.children[last].keys.len() < T {
            Self::fill_child(node, last);
        }
        let last = node.children.len() - 1;
        Self::remove_max(&mut node.children[last])
    }

    fn remove_min(node: &mut BTreeNode<K, V>) -> (K, V) {
        if node.leaf {
            let k = node.keys.remove(0);
            let v = node.values.remove(0);
            return (k, v);
        }
        if node.children[0].keys.len() < T {
            Self::fill_child(node, 0);
        }
        Self::remove_min(&mut node.children[0])
    }

    /// Ensure child[i] has at least t keys by rotating or merging.
    fn fill_child(node: &mut BTreeNode<K, V>, i: usize) {
        // Try borrowing from left sibling
        if i > 0 && node.children[i - 1].keys.len() >= T {
            Self::rotate_right(node, i);
        }
        // Try borrowing from right sibling
        else if i < node.children.len() - 1 && node.children[i + 1].keys.len() >= T {
            Self::rotate_left(node, i);
        }
        // Merge with a sibling
        else if i > 0 {
            Self::merge_children(node, i - 1);
        } else {
            Self::merge_children(node, i);
        }
    }

    fn rotate_right(parent: &mut BTreeNode<K, V>, i: usize) {
        // Move parent.keys[i-1] down to child[i], move last key of child[i-1] up
        let parent_key = parent.keys[i - 1].clone();
        let parent_val = parent.values[i - 1].clone();

        let left = &mut parent.children[i - 1];
        let borrowed_key = left.keys.pop().unwrap();
        let borrowed_val = left.values.pop().unwrap();
        let borrowed_child = if !left.leaf { Some(left.children.pop().unwrap()) } else { None };

        parent.keys[i - 1] = borrowed_key;
        parent.values[i - 1] = borrowed_val;

        let right = &mut parent.children[i];
        right.keys.insert(0, parent_key);
        right.values.insert(0, parent_val);
        if let Some(child) = borrowed_child {
            right.children.insert(0, child);
        }
    }

    fn rotate_left(parent: &mut BTreeNode<K, V>, i: usize) {
        let parent_key = parent.keys[i].clone();
        let parent_val = parent.values[i].clone();

        let right = &mut parent.children[i + 1];
        let borrowed_key = right.keys.remove(0);
        let borrowed_val = right.values.remove(0);
        let borrowed_child = if !right.leaf { Some(right.children.remove(0)) } else { None };

        parent.keys[i] = borrowed_key;
        parent.values[i] = borrowed_val;

        let left = &mut parent.children[i];
        left.keys.push(parent_key);
        left.values.push(parent_val);
        if let Some(child) = borrowed_child {
            left.children.push(child);
        }
    }

    fn merge_children(parent: &mut BTreeNode<K, V>, i: usize) {
        // Merge child[i+1] into child[i], pulling parent.keys[i] down
        let separator_key = parent.keys.remove(i);
        let separator_val = parent.values.remove(i);
        let right = parent.children.remove(i + 1);

        let left = &mut parent.children[i];
        left.keys.push(separator_key);
        left.values.push(separator_val);
        left.keys.extend(right.keys);
        left.values.extend(right.values);
        left.children.extend(right.children);
    }

    fn len(&self) -> usize {
        Self::count_keys(&self.root)
    }

    fn count_keys(node: &BTreeNode<K, V>) -> usize {
        let mut count = node.keys.len();
        for child in &node.children { count += Self::count_keys(child); }
        count
    }
}

fn main() {
    let mut tree = BTree::new();
    for i in 0..100u32 {
        tree.insert(i, format!("val-{}", i));
    }
    println!("Before deletion: {} keys", tree.len());

    // Delete several keys
    for &k in &[0u32, 50, 99, 25, 75, 42] {
        let result = tree.delete(&k);
        println!("Delete {}: {:?}", k, result);
    }
    println!("After deletion: {} keys", tree.len());

    // Verify deleted keys are gone
    for &k in &[0u32, 50, 99, 25, 75, 42] {
        assert!(tree.search(&k).is_none(), "key {} should be deleted", k);
    }

    // Verify remaining keys are still present
    for i in 0..100u32 {
        if ![0, 50, 99, 25, 75, 42].contains(&i) {
            assert!(tree.search(&i).is_some(), "key {} should exist", i);
        }
    }
    println!("All assertions passed -- deletion is correct!");
}
```

</details>

---

## Recap

A B-tree is a self-balancing search tree where each node holds multiple keys in sorted order. It gives you O(log n) exact lookups -- slower than a hash table's O(1), but it gives you something a hash table never can: order. Range queries, sorted iteration, min, max, prefix scans -- all natural, all efficient.

The B-tree maintains balance through splits: when a node overflows, it splits in half and pushes the median key upward. The tree only grows taller from the root, so all leaves stay at the same depth. For a minimum degree of t, the maximum height for n keys is log_t(n), making every operation logarithmic.

In our database, `BTreeMap` powers the `MemoryStorage` engine. In the real world, B-trees power nearly every relational database index on the planet. Understanding how they work -- the splits, the balance invariant, the range traversal -- is understanding how databases have organized data since 1972.
