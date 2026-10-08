export interface Student {
  readonly name: string;
  readonly scores: readonly number[];
}

export function student(name: string, scores: readonly number[]): Student {
  return { name, scores };
}

// Integer division rounding down; throws on a zero divisor.
export function floorDiv(a: number, b: number): number {
  if (b === 0) throw new RangeError("division by zero");
  return Math.floor(a / b);
}

export function average(s: Student): number {
  if (s.scores.length === 0) return 0;
  return floorDiv(s.scores.reduce((a, b) => a + b, 0), s.scores.length);
}

export function letter(avg: number): string {
  if (avg >= 90) return "A";
  if (avg >= 80) return "B";
  if (avg >= 70) return "C";
  if (avg >= 60) return "D";
  return "F";
}

export function grade(s: Student): string {
  return letter(average(s));
}

function cmpStr(a: string, b: string): number {
  return a < b ? -1 : a > b ? 1 : 0;
}

export function ranking(ss: readonly Student[]): string[] {
  return [...ss].sort((a, b) => average(b) - average(a) || cmpStr(a.name, b.name)).map((s) => s.name);
}

export function dropLowest(s: Student): Student {
  if (s.scores.length < 2) return s;
  const rest = [...s.scores];
  rest.splice(rest.indexOf(Math.min(...rest)), 1);
  return student(s.name, rest);
}

export function curve(ss: readonly Student[], pts: number): Student[] {
  return ss.map((s) => student(s.name, s.scores.map((x) => Math.min(x + pts, 100))));
}

export function honorRoll(ss: readonly Student[]): string[] {
  return ss.filter((s) => grade(dropLowest(s)) === "A").map((s) => s.name).sort(cmpStr);
}

export function reportCard(s: Student): string {
  return `${s.name}: ${average(s)} (${grade(s)})`;
}

export function roster(): Student[] {
  return [
    student("mia", [88, 92, 95]),
    student("ali", [70, 65, 80]),
    student("zed", [95, 90, 50]),
    student("bo", [100, 92, 89]),
  ];
}
