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

export class TaskErr extends Error {}

export class NotFound extends TaskErr {
  constructor(readonly id: number) {
    super(`no task ${id}`);
  }
}

export class AlreadyDone extends TaskErr {
  constructor(readonly id: number) {
    super(`task ${id} already done`);
  }
}

export function task(id: number, title: string, done: boolean, pri: Priority): Task {
  return { id, title, done, pri };
}

export function findTask(ts: readonly Task[], id: number): Task {
  const t = ts.find((t) => t.id === id);
  if (t === undefined) throw new NotFound(id);
  return t;
}

export function nextId(ts: readonly Task[]): number {
  return ts.reduce((m, t) => Math.max(m, t.id), 0) + 1;
}

export function addTask(ts: readonly Task[], title: string, pri: Priority): Task[] {
  return [...ts, task(nextId(ts), title, false, pri)];
}

export function complete(ts: readonly Task[], id: number): Task[] {
  if (findTask(ts, id).done) throw new AlreadyDone(id);
  return ts.map((t) => (t.id === id ? { ...t, done: true } : t));
}

export function retitle(ts: readonly Task[], id: number, title: string): Task[] {
  findTask(ts, id);
  return ts.map((t) => (t.id === id ? { ...t, title } : t));
}

export function clearDone(ts: readonly Task[]): Task[] {
  return ts.filter((t) => !t.done);
}

const RANK: Record<Priority, number> = { [Priority.High]: 0, [Priority.Med]: 1, [Priority.Low]: 2 };

export function byPriority(ts: readonly Task[]): Task[] {
  return pending(ts).sort((a, b) => RANK[a.pri] - RANK[b.pri] || a.id - b.id);
}

export function pending(ts: readonly Task[]): Task[] {
  return ts.filter((t) => !t.done);
}

export function box(t: Task): string {
  return t.done ? "[x]" : "[ ]";
}

export function render(t: Task): string {
  return `${box(t)} #${t.id} (${t.pri}) ${t.title}`;
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
