/** The items within `reach` of index `at`, wrapping at both ends, with their signed distance from it. */
export function around<T>(items: readonly T[], at: number, reach: number): { item: T; offset: number }[] {
  const n = items.length;
  const out: { item: T; offset: number }[] = [];
  for (let offset = -reach; offset <= reach; offset++) {
    out.push({ item: items[(((at + offset) % n) + n) % n], offset });
  }
  return out;
}
