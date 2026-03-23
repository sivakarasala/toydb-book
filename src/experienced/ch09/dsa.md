## DSA in Context: Tree Transformations

The optimizer performs tree transformations — it takes a plan tree as input and produces a modified plan tree as output. This is a fundamental operation in computer science, appearing in compilers, interpreters, symbolic math engines, and document processors.

### Pattern matching on tree nodes

Each optimizer rule is a pattern matcher. It looks at a node in the tree and asks: "Does this node match a pattern I can optimize?" If yes, it rewrites the node. If no, it recurses into the children.

```
Constant Folding Pattern:
  Match:   BinaryOp(Literal(a), op, Literal(b))
  Rewrite: Literal(eval(a, op, b))

Filter Pushdown Pattern:
  Match:   Filter(pred, Project(cols, source))
           where pred references only columns in cols
  Rewrite: Project(cols, Filter(pred, source))
```

This is exactly how compilers work. GCC and LLVM have hundreds of optimization passes, each one a pattern matcher that looks for specific tree shapes and rewrites them. The key insight is that each pass is simple — it handles one pattern. The power comes from composing many simple passes.

### Fixed-point iteration

Our optimizer applies each rule once. A more sophisticated approach is fixed-point iteration: apply all rules repeatedly until no rule makes any changes.

```rust
/// Apply rules repeatedly until the plan stops changing.
/// This is called "fixed-point iteration" — we iterate until
/// we reach a fixed point (a state that does not change).
fn optimize_to_fixed_point(&self, plan: Plan) -> OptimizeResult {
    let mut current = plan;
    let mut all_applied: Vec<String> = Vec::new();
    let mut iterations = 0;
    let max_iterations = 100; // Safety limit

    loop {
        let before = format!("{:?}", current);
        let mut changed = false;

        for rule in &self.rules {
            let before_rule = format!("{:?}", current);
            current = rule.optimize(current);
            let after_rule = format!("{:?}", current);

            if before_rule != after_rule {
                all_applied.push(rule.name().to_string());
                changed = true;
            }
        }

        iterations += 1;
        if !changed || iterations >= max_iterations {
            break;
        }
    }

    OptimizeResult {
        plan: current,
        applied_rules: all_applied,
    }
}
```

Why would rules need multiple passes? Consider filter pushdown through two levels of project:

```
Pass 1:
  Filter(pred)              Project(cols1)
    Project(cols1)    =>      Filter(pred)
      Project(cols2)            Project(cols2)
        Scan                      Scan

Pass 2:
  Project(cols1)            Project(cols1)
    Filter(pred)      =>      Project(cols2)
      Project(cols2)            Filter(pred)
        Scan                      Scan
```

The first pass pushes the filter past the first project. The second pass pushes it past the second project. Each pass applies a simple, local transformation. Multiple passes achieve a global result.

Fixed-point iteration is guaranteed to terminate if each rule either makes progress (reduces some measure of the plan) or leaves it unchanged. If a rule could increase the measure, you might loop forever — hence the safety limit.

### Time complexity

Our optimizer has O(R * N) time complexity per pass, where R is the number of rules and N is the number of nodes in the plan tree. Each rule walks the entire tree once. With fixed-point iteration, the worst case is O(I * R * N) where I is the number of iterations, but in practice I is small (typically 2-5) because each pass resolves most opportunities.

Production optimizers like PostgreSQL's use more sophisticated algorithms. They precompute which rules can fire based on the types of nodes present, so they skip rules that cannot possibly match. But the fundamental structure — a collection of transformation rules applied to a tree — is the same.

---
