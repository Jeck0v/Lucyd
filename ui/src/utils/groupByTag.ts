/**
 * Groups an array of tagged items into a `Map<tag, items[]>`.
 *
 * Items that carry no `tags` field (or an empty array) are placed under the
 * implicit `"default"` group. Only the first tag of each item is used for
 * grouping; this mirrors the common convention where the first tag is the
 * primary category.
 */
export function groupByTag<T extends { tags?: string[] }>(
  items: T[],
): Map<string, T[]> {
  const groups = new Map<string, T[]>()

  for (const item of items) {
    const tag = item.tags?.[0] ?? 'default'
    const group = groups.get(tag) ?? []
    group.push(item)
    groups.set(tag, group)
  }

  return groups
}
