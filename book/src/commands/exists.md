# exists

O(1) bloom filter existence check. Returns true or false in near-zero tokens.

## Usage

```bash
prx exists <pattern> [path]
```

## Examples

```bash
# Does "authenticate" appear anywhere in src/?
prx exists "authenticate" src/

# Does this specific string exist?
prx exists "redis" src/
```

Output:

```json
{
  "data": {
    "exists": true
  }
}
```

## How it works

`prx exists` uses a bloom filter built during `prx index`. The check is O(1) regardless of codebase size. Without an index, it falls back to a fast scan.

Bloom filters have no false negatives: if `exists` returns false, the pattern definitely isn't there. They can have false positives: if it returns true, the pattern is very likely there (but do a full search to confirm).

## Tips

- Use `prx exists` before `prx search` when you just need a yes/no. It costs near-zero tokens vs the full search cost.
- The typical pattern: `prx exists "redis" src/` to check if Redis is used at all, then `prx search "redis" src/` only if it is.
- `prx exists` is most useful for large codebases where a full search would be expensive.

See also: [search](search.md), [index](index.md)
