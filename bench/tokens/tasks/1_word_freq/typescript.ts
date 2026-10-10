export function topWords(text: string, k: number): [string, number][] {
  const counts = new Map<string, number>();
  for (const w of text.toLowerCase().match(/[a-z]+/g) ?? []) {
    counts.set(w, (counts.get(w) ?? 0) + 1);
  }
  return [...counts.entries()]
    .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
    .slice(0, k);
}
