export enum Priority {
  Low = "low",
  Med = "med",
  High = "high",
}

export interface Task {
  readonly id: number;
  readonly title: string;
  readonly done: boolean;
  readonly pri: Priority;
}

export function task(id: number, title: string, done: boolean, pri: Priority): Task {
  return { id, title, done, pri };
}

export function nextId(ts: readonly Task[]): number {
  return ts.reduce((m, t) => Math.max(m, t.id), 0) + 1;
}

export function addTask(ts: readonly Task[], title: string, pri: Priority): Task[] {
  return [...ts, task(nextId(ts), title, false, pri)];
}

export function complete(ts: readonly Task[], id: number): Task[] {
  return ts.map((t) => (t.id === id ? { ...t, done: true } : t));
}

export function pending(ts: readonly Task[]): Task[] {
  return ts.filter((t) => !t.done);
}

export function box(t: Task): string {
  return t.done ? "[x]" : "[ ]";
}

export function render(t: Task): string {
  return `${box(t)} #${t.id} ${t.title}`;
}

export function renderAll(ts: readonly Task[]): string {
  return ts.map(render).join("\n");
}

export function demo(): Task[] {
  let ts: Task[] = [];
  ts = addTask(ts, "buy milk", Priority.Low);
  ts = addTask(ts, "write report", Priority.High);
  ts = addTask(ts, "call bob", Priority.Med);
  ts = addTask(ts, "fix sink", Priority.High);
  return ts;
}
