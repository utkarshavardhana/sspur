export type Pair = [string, number];

export function wordsOf(text: string): string[] {
  return text.toLowerCase().match(/[a-z0-9]+/g) ?? [];
}

export function wordFreq(text: string): Pair[] {
  const counts = new Map<string, number>();
  for (const w of wordsOf(text)) counts.set(w, (counts.get(w) ?? 0) + 1);
  return [...counts.entries()];
}

export function topK(text: string, k: number): Pair[] {
  if (k < 0) throw new RangeError("k must be >= 0");
  return wordFreq(text).sort((p, q) => q[1] - p[1]).slice(0, k);
}

export function fmtPair(p: Pair): string {
  return `${p[0]}=${p[1]}`;
}

export function topLine(text: string, k: number): string {
  return topK(text, k).map(fmtPair).join(" ");
}

export function summary(text: string): string {
  const n = wordsOf(text).length;
  const d = wordFreq(text).length;
  return `${n} words, ${d} distinct, top: ${topLine(text, 3)}`;
}
