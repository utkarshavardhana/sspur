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
  return floorDiv(s.scores.reduce((a, b) => a + b, 0), s.scores.length);
}

export function letter(avg: number): string {
  if (avg > 90) return "A";
  if (avg > 80) return "B";
  if (avg > 70) return "C";
  if (avg > 60) return "D";
  return "F";
}

export function grade(s: Student): string {
  return letter(average(s));
}

export function ranking(ss: readonly Student[]): string[] {
  return [...ss].sort((a, b) => average(b) - average(a)).map((s) => s.name);
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
