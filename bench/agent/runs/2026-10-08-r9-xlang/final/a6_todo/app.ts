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

export class TaskErr extends Error {
  constructor(message: string) {
    super(message);
    this.name = new.target.name;
    Object.setPrototypeOf(this, new.target.prototype);
  }
}

export class NotFound extends TaskErr {
  readonly id: number;
  constructor(id: number) {
    super(`task ${id} not found`);
    this.id = id;
  }
}

export class AlreadyDone extends TaskErr {
  readonly id: number;
  constructor(id: number) {
    super(`task ${id} already done`);
    this.id = id;
  }
}

export function complete(ts: readonly Task[], id: number): Task[] {
  const t = ts.find((x) => x.id === id);
  if (t === undefined) throw new NotFound(id);
  if (t.done) throw new AlreadyDone(id);
  return ts.map((x) => (x.id === id ? { ...x, done: true } : x));
}

const PRI_RANK: Record<Priority, number> = {
  [Priority.High]: 0,
  [Priority.Med]: 1,
  [Priority.Low]: 2,
};

export function byPriority(ts: readonly Task[]): Task[] {
  return pending(ts).sort((a, b) => PRI_RANK[a.pri] - PRI_RANK[b.pri] || a.id - b.id);
}

export function clearDone(ts: readonly Task[]): Task[] {
  return ts.filter((t) => !t.done);
}

export function retitle(ts: readonly Task[], id: number, title: string): Task[] {
  if (!ts.some((t) => t.id === id)) throw new NotFound(id);
  return ts.map((t) => (t.id === id ? { ...t, title } : t));
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
