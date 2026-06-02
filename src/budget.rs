/// Retain items from `vec` while their cumulative cost stays within `budget`.
/// Returns the total cost of retained items.
pub fn retain_within<T>(vec: &mut Vec<T>, budget: usize, cost_fn: impl Fn(&T) -> usize) -> usize {
    let mut used = 0;
    vec.retain(|item| {
        let cost = cost_fn(item);
        if used + cost <= budget {
            used += cost;
            true
        } else {
            false
        }
    });
    used
}
