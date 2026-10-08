export type Pair = [string, number];

export function wordsOf(text: string, stop: readonly string[]): string[] {
  return (text.toLowerCase().match(/[a-z0-9]+/g) ?? []).filter((w) => !stop.includes(w));
}

export function wordFreq(text: string, stop: readonly string[]): Pair[] {
  const counts = new Map<string, number>();
  for (const w of wordsOf(text, stop)) counts.set(w, (counts.get(w) ?? 0) + 1);
  return [...counts.entries()];
}

export function topK(text: string, k: number, stop: readonly string[]): Pair[] {
  if (k < 0) throw new RangeError("k must be >= 0");
  return wordFreq(text, stop)
    .sort((p, q) => q[1] - p[1] || (p[0] < q[0] ? -1 : p[0] > q[0] ? 1 : 0))
    .slice(0, k);
}

export function fmtPair(p: Pair): string {
  return `${p[0]}=${p[1]}`;
}

export function topLine(text: string, k: number, stop: readonly string[]): string {
  return topK(text, k, stop).map(fmtPair).join(" ");
}

export function summary(text: string, stop: readonly string[]): string {
  const n = wordsOf(text, stop).length;
  const d = wordFreq(text, stop).length;
  return `${n} words, ${d} distinct, top: ${topLine(text, 3, stop)}`;
}

export function longestWord(text: string): string | undefined {
  let best: string | undefined;
  for (const w of wordsOf(text, [])) {
    if (best === undefined || w.length > best.length) best = w;
  }
  return best;
}
